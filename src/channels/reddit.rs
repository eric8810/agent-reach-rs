//! Reddit — multi-backend: native API / OpenCLI / rdt-cli.
//!
//! Honest tiering (live-verified 2026-06): there is NO zero-config path.
//! Anonymous .json endpoints are blocked (403 anti-bot), and the official API
//! closed self-service registration in 2025-11 (manual approval, individual
//! scripts rarely granted). Every working backend rides a logged-in session:
//! native API uses a reddit_session cookie from config, OpenCLI reuses the
//! browser's, rdt-cli imports cookies.
//!
//! Backend order: Reddit API (native) → OpenCLI → rdt-cli

use crate::backends::opencli_status;
use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;

use serde_json::Value;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Credential file for rdt-cli.
const CREDENTIAL_FILE: &str = "~/.config/rdt-cli/credential.json";
/// Pinned to the 0.4.2 state — PyPI still only has 0.4.1 (upstream issue #10).
const RDT_GIT_SOURCE: &str =
    "git+https://github.com/public-clis/rdt-cli.git@5e4fb3720d5c174e976cd425ccc3b879d52cac66";

/// Shell exit codes for "found but not executable" / "not found"
/// (aligned with agent_reach.probe).
const BROKEN_EXIT_CODES: [i32; 2] = [126, 127];

/// rdt should be installed from the pinned git source (PyPI is behind).
const RDT_BROKEN_HINT: &str =
    "rdt 命令存在但无法执行——通常是系统 Python 升级后 venv 解释器丢失。\n\
     PyPI 版本落后，推荐用固定 git 源强制重装：\n\
       pipx install --force 'git+https://github.com/public-clis/rdt-cli.git@5e4fb3720d5c174e976cd425ccc3b879d52cac66'";

/// User-Agent for Reddit JSON API requests.
const REDDIT_UA: &str = "agent-reach/1.5.0 (by /u/your_agent)";

/// Reddit API base for JSON endpoints.
const REDDIT_BASE: &str = "https://www.reddit.com";

/// Channel for Reddit posts and comments.
pub struct RedditChannel {
    active_backend: Option<String>,
    /// Cached ureq::Agent for connection pooling (built once per check).
    reddit_agent: Option<ureq::Agent>,
    /// Reddit session cookie value (from config), set as Cookie header on requests.
    reddit_cookie: Option<String>,
}

impl RedditChannel {
    pub fn new() -> Self {
        RedditChannel {
            active_backend: None,
            reddit_agent: None,
            reddit_cookie: None,
        }
    }

    // ── native API helpers ─────────────────────────────────────────

    /// Extract the reddit_session cookie value from config.
    /// Tries `reddit_cookie` key first, then `reddit_session`.
    /// Supports raw token values and rdt-cli JSON credential file format.
    fn extract_cookie(config: Option<&Config>) -> Option<String> {
        let cookie_val = config.and_then(|c| {
            c.get("reddit_cookie")
                .or_else(|| c.get("reddit_session"))
        })?;
        let trimmed = cookie_val.trim().to_string();
        if trimmed.is_empty() {
            return None;
        }

        // Try to parse as JSON (rdt-cli credential file format)
        if let Ok(parsed) = serde_json::from_str::<Value>(&trimmed) {
            if let Some(s) = parsed
                .get("cookies")
                .and_then(|c| c.get("reddit_session"))
                .and_then(|v| v.as_str())
            {
                return Some(s.to_string());
            }
            if let Some(s) = parsed.get("reddit_session").and_then(|v| v.as_str()) {
                return Some(s.to_string());
            }
        }

        // Fallback: treat as raw cookie value
        Some(trimmed)
    }

    /// Build a `ureq::Agent` for connection pooling.
    fn build_agent() -> ureq::Agent {
        ureq::AgentBuilder::new().build()
    }

    /// Issue a GET request to a Reddit JSON endpoint.
    fn reddit_get(
        agent: &ureq::Agent,
        cookie: Option<&str>,
        path: &str,
    ) -> Result<Value, String> {
        let url_str = format!("{}{}", REDDIT_BASE, path);
        let mut req = agent
            .get(&url_str)
            .set("User-Agent", REDDIT_UA)
            .set("Accept", "application/json")
            .timeout(Duration::from_secs(30));
        if let Some(c) = cookie {
            req = req.set("Cookie", &format!("reddit_session={}", c));
        }
        match req.call() {
            Ok(resp) => {
                let body = resp
                    .into_string()
                    .map_err(|e| format!("Reddit API read error: {}", e))?;
                serde_json::from_str(&body)
                    .map_err(|e| format!("Reddit API JSON parse error: {}", e))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body = resp
                    .into_string()
                    .unwrap_or_else(|_| "<unreadable>".to_string());
                Err(format!(
                    "Reddit API returned HTTP {}: {}",
                    code,
                    body.chars().take(500).collect::<String>()
                ))
            }
            Err(ureq::Error::Transport(e)) => {
                Err(format!("Reddit API transport error: {}", e))
            }
        }
    }

    // ── public data-fetching methods ────────────────────────────────

    /// Search Reddit for posts matching a query.
    pub fn search_reddit(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let agent = self
            .reddit_agent
            .as_ref()
            .ok_or("Reddit native backend not initialized (call check() first)")?;
        let path = format!(
            "/search.json?q={}&limit={}&type=link",
            urlencoding(query),
            limit
        );
        let data = Self::reddit_get(agent, self.reddit_cookie.as_deref(), &path)?;
        extract_children(&data)
            .ok_or_else(|| "Reddit search: unexpected response shape".to_string())
    }

    /// Get posts from a subreddit.
    pub fn get_subreddit_posts(
        &self,
        subreddit: &str,
        sort: &str,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let agent = self
            .reddit_agent
            .as_ref()
            .ok_or("Reddit native backend not initialized (call check() first)")?;
        let path = format!(
            "/r/{}/{}.json?limit={}",
            subreddit.trim_matches('/').trim(),
            sort,
            limit
        );
        let data = Self::reddit_get(agent, self.reddit_cookie.as_deref(), &path)?;
        extract_children(&data)
            .ok_or_else(|| "Reddit subreddit: unexpected response shape".to_string())
    }

    /// Get comments for a post.
    pub fn get_post_comments(&self, post_id: &str) -> Result<Value, String> {
        let agent = self
            .reddit_agent
            .as_ref()
            .ok_or("Reddit native backend not initialized (call check() first)")?;
        let path = format!("/comments/{}.json", post_id);
        Self::reddit_get(agent, self.reddit_cookie.as_deref(), &path)
    }

    /// Read a Reddit post URL by extracting the post ID and fetching its JSON.
    pub fn read_post(&self, url: &str) -> Result<Value, String> {
        let agent = self
            .reddit_agent
            .as_ref()
            .ok_or("Reddit native backend not initialized (call check() first)")?;

        // Parse the post ID from the URL: /r/{sub}/comments/{id}/...
        let parsed =
            url::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;
        let path = parsed.path();

        // Try /r/{sub}/comments/{id}/... pattern
        if let Some(rest) = path.strip_prefix("/r/") {
            if let Some(remainder) = rest.split('/').nth(2) {
                if remainder == "comments" {
                    if let Some(post_id) = rest.split('/').nth(3) {
                        if !post_id.is_empty() {
                            let json_path = format!("/comments/{}.json", post_id);
                            return Self::reddit_get(
                                agent,
                                self.reddit_cookie.as_deref(),
                                &json_path,
                            );
                        }
                    }
                }
            }
        }

        // Try /comments/{id} directly
        if let Some(rest) = path.strip_prefix("/comments/") {
            let post_id = rest.split('/').next().unwrap_or("");
            if !post_id.is_empty() {
                let json_path = format!("/comments/{}.json", post_id);
                return Self::reddit_get(
                    agent,
                    self.reddit_cookie.as_deref(),
                    &json_path,
                );
            }
        }

        // Fallback: add .json suffix and fetch
        let json_path = if path.ends_with('/') {
            format!("{}.json", path.trim_end_matches('/'))
        } else {
            format!("{}.json", path)
        };
        Self::reddit_get(agent, self.reddit_cookie.as_deref(), &json_path)
    }

    // ── native backend check ───────────────────────────────────────

    /// Probe the native Reddit API backend.
    fn check_native(&self, config: Option<&Config>) -> Option<(CheckStatus, String)> {
        let agent = Self::build_agent();
        let cookie = Self::extract_cookie(config);
        let cookie_for_req = cookie.as_deref();

        // Lightweight probe: fetch 1 post from r/rust
        let url = format!("{}/r/rust/hot.json?limit=1", REDDIT_BASE);
        let mut req = agent
            .get(&url)
            .set("User-Agent", REDDIT_UA)
            .set("Accept", "application/json")
            .timeout(Duration::from_secs(15));
        if let Some(c) = cookie_for_req {
            req = req.set("Cookie", &format!("reddit_session={}", c));
        }
        let result = req.call();

        match result {
            Ok(resp) => {
                let status_code = resp.status();
                if status_code == 200 {
                    return Some((
                        CheckStatus::Ok,
                        "Reddit API (native) 可用 — 通过 reddit_session Cookie 直接调用 .json 端点".to_string(),
                    ));
                }
                // Unexpected success code — treat as ok
                if status_code < 400 {
                    return Some((
                        CheckStatus::Ok,
                        format!(
                            "Reddit API (native) 可用 (HTTP {}) — 直接调用 .json 端点",
                            status_code
                        ),
                    ));
                }
                let body = resp
                    .into_string()
                    .unwrap_or_else(|_| "<unreadable>".to_string());
                let snippet = body.chars().take(200).collect::<String>();
                Some((
                    CheckStatus::Warn,
                    format!(
                        "Reddit API (native) 返回 HTTP {} (需要有效的 reddit_session Cookie)。\n\
                         {}",
                        status_code, snippet
                    ),
                ))
            }
            Err(ureq::Error::Status(403, _)) => Some((
                CheckStatus::Warn,
                concat!(
                    "Reddit API (native) 返回 403 — 需要登录态 Cookie。\n\n\
                     设置方法：\n  \
                     1. 在浏览器登录 reddit.com\n  \
                     2. 安装 Cookie-Editor 扩展：\n    \
                     https://chromewebstore.google.com/detail/cookie-editor/hlkenndednhfkekhgcdicdfddnkalmdm\n  \
                     3. 点击 Cookie-Editor → 找到 reddit_session → 复制 Value\n  \
                     4. 运行: agent-reach config set reddit_cookie \"<粘贴 Value>\""
                )
                .to_string(),
            )),
            Err(ureq::Error::Status(code, _)) => Some((
                CheckStatus::Warn,
                format!(
                    "Reddit API (native) 返回 HTTP {} — 可能需要有效的 Cookie。\n\
                     运行: agent-reach config set reddit_cookie \"<你的 reddit_session>\"",
                    code
                ),
            )),
            Err(ureq::Error::Transport(e)) => {
                let msg = e.to_string();
                // Network-level failure: unreachable, timeout, DNS, TLS, etc.
                // Don't report as warn — fall through to next backend.
                if msg.contains("timeout") || msg.contains("timed out") {
                    Some((
                        CheckStatus::Error,
                        "Reddit API (native) 连接超时 — 可能需要代理。".to_string(),
                    ))
                } else {
                    // Connection refused, DNS failure, etc. — let fallbacks try.
                    None
                }
            }
        }
    }

    // ── OpenCLI backend ────────────────────────────────────────────

    /// OpenCLI candidate. None = not installed.
    fn check_opencli(&self) -> Option<(CheckStatus, String)> {
        let st = opencli_status(10);
        if !st.installed {
            return None;
        }
        if st.broken {
            return Some((CheckStatus::Error, st.hint));
        }
        if st.ready() {
            return Some((
                CheckStatus::Ok,
                "OpenCLI 可用（复用浏览器登录态）。用法：\
                 opencli reddit search/read/subreddit/hot -f yaml"
                    .to_string(),
            ));
        }
        Some((CheckStatus::Warn, st.hint))
    }

    // ── rdt-cli backend ────────────────────────────────────────────

    /// rdt-cli candidate. None = not installed.
    fn check_rdt(&self) -> Option<(CheckStatus, String)> {
        let rdt_path = match which::which("rdt") {
            Ok(p) => p,
            Err(_) => return None,
        };

        let child = match Command::new(&rdt_path)
            .arg("status")
            .arg("--json")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Some((CheckStatus::Error, RDT_BROKEN_HINT.to_string()));
                }
                return Some((CheckStatus::Error, RDT_BROKEN_HINT.to_string()));
            }
        };

        let child_pid = child.id();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let result = child.wait_with_output();
            let _ = tx.send(result);
        });

        let output = match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(out)) => out,
            Ok(Err(_)) => {
                return Some((
                    CheckStatus::Error,
                    "rdt 进程等待失败，Reddit 状态未知。运行 `rdt status` 查看详情".to_string(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Some((
                    CheckStatus::Error,
                    "rdt 进程异常退出，Reddit 状态未知。运行 `rdt status` 查看详情"
                        .to_string(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                {
                    let pid = child_pid;
                    #[cfg(windows)]
                    {
                        let _ = Command::new("taskkill")
                            .args(["/F", "/PID", &pid.to_string()])
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn();
                    }
                    #[cfg(not(windows))]
                    {
                        let _ = Command::new("kill")
                            .args(["-9", &pid.to_string()])
                            .stdout(Stdio::null())
                            .stderr(Stdio::null())
                            .spawn();
                    }
                }
                return Some((
                    CheckStatus::Error,
                    "rdt 响应超时（>10s），Reddit 状态未知。稍后重试或运行 `rdt status` 查看详情"
                        .to_string(),
                ));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);

        if let Some(code) = output.status.code() {
            if BROKEN_EXIT_CODES.contains(&code) {
                return Some((CheckStatus::Error, RDT_BROKEN_HINT.to_string()));
            }
            if code != 0 {
                let detail: Vec<&str> = stderr_str
                    .trim()
                    .lines()
                    .chain(stdout.trim().lines())
                    .collect();
                let tail = detail.last().copied().unwrap_or("无输出");
                return Some((
                    CheckStatus::Error,
                    format!(
                        "rdt 异常退出（exit {}）：{}。运行 `rdt status` 查看详情",
                        code, tail
                    ),
                ));
            }
        }

        // Parse JSON status
        #[derive(serde::Deserialize)]
        struct RdtStatusResponse {
            data: Option<RdtStatusData>,
        }
        #[derive(serde::Deserialize)]
        struct RdtStatusData {
            authenticated: Option<bool>,
            username: Option<String>,
        }

        let data: Option<RdtStatusResponse> = serde_json::from_str(stdout.trim()).ok();

        let info = match data {
            Some(d) => d.data.unwrap_or(RdtStatusData {
                authenticated: None,
                username: None,
            }),
            None => {
                return Some((
                    CheckStatus::Warn,
                    "rdt-cli 可用但状态输出无法解析，运行 `rdt status` 查看登录状态"
                        .to_string(),
                ));
            }
        };

        let authenticated = info.authenticated.unwrap_or(false);
        let username = info.username.unwrap_or_default();

        if authenticated {
            let suffix = if !username.is_empty() {
                format!("（已登录：{}）", username)
            } else {
                String::new()
            };
            return Some((
                CheckStatus::Ok,
                format!(
                    "rdt-cli 可用{}（搜索帖子、阅读全文、查看评论；\
                     上游 2026-03 起停更，桌面用户建议迁移到 OpenCLI）",
                    suffix
                ),
            ));
        }

        Some((
            CheckStatus::Warn,
            format!(
                "rdt-cli 已安装但未登录。Reddit 自 2024 年起要求认证，\
                 未登录时所有请求均返回 403。\n\n\
                 方法一（自动）：运行 `rdt login`\n  \
                 先在浏览器登录 reddit.com，再运行此命令自动提取 Cookie。\n\n\
                 方法二（手动，适用于 Chrome/Edge 127+ 无法自动提取时）：\n  \
                 1. Chrome 应用商店安装 Cookie-Editor 扩展：\n    \
                 https://chromewebstore.google.com/detail/cookie-editor/hlkenndednhfkekhgcdicdfddnkalmdm\n  \
                 2. 在浏览器打开 reddit.com（确保已登录）\n  \
                 3. 点击 Cookie-Editor 图标，找到 `reddit_session`，复制其 Value\n  \
                 4. 将以下内容写入 {}：\n    \
                 {{\"cookies\": {{\"reddit_session\": \"<粘贴 Value>\"}}, \
                 \"source\": \"manual\", \"username\": \"<你的用户名>\", \
                 \"modhash\": null, \"saved_at\": 0, \"last_verified_at\": null}}\n\n\
                 验证：`rdt status --json` 确认 authenticated: true",
                CREDENTIAL_FILE
            ),
        ))
    }
}

impl Channel for RedditChannel {
    fn name(&self) -> &str {
        "reddit"
    }

    fn description(&self) -> &str {
        "Reddit 帖子和评论"
    }

    fn backends(&self) -> &[&str] {
        &["Reddit API (native)", "OpenCLI", "rdt-cli"]
    }

    fn tier(&self) -> u8 {
        1 // no zero-config path exists — see module docstring
    }

    fn can_handle(&self, url_str: &str) -> bool {
        match url::Url::parse(url_str) {
            Ok(u) => {
                let host = u.host_str().unwrap_or("").to_lowercase();
                host.contains("reddit.com") || host.contains("redd.it")
            }
            Err(_) => false,
        }
    }

    fn active_backend(&self) -> Option<String> {
        self.active_backend.clone()
    }

    fn set_active_backend(&mut self, backend: Option<String>) {
        self.active_backend = backend;
    }

    fn check(&mut self, config: Option<&Config>) -> CheckResult {
        self.active_backend = None;
        self.reddit_agent = None;
        self.reddit_cookie = None;
        let mut findings: Vec<(String, CheckStatus, String)> = Vec::new();

        for backend in self.ordered_backends(config) {
            let result = match backend.as_str() {
                "Reddit API (native)" => {
                    let r = self.check_native(config);
                    // Cache the agent and cookie so data-fetching methods can use them
                    if r.as_ref().map_or(false, |(s, _)| *s == CheckStatus::Ok) {
                        self.reddit_agent = Some(Self::build_agent());
                        self.reddit_cookie = Self::extract_cookie(config);
                    }
                    r
                }
                "OpenCLI" => self.check_opencli(),
                "rdt-cli" => self.check_rdt(),
                _ => continue,
            };

            if let Some((status, message)) = result {
                findings.push((backend, status, message));
            }
        }

        // First "ok" wins.
        for wanted in &[CheckStatus::Ok, CheckStatus::Warn] {
            for (backend, status, message) in &findings {
                if status == wanted {
                    self.active_backend = Some(backend.clone());
                    return CheckResult {
                        status: *status,
                        message: message.clone(),
                        active_backend: self.active_backend.clone(),
                    };
                }
            }
        }

        // Only broken/timeout/error candidates remain.
        if !findings.is_empty() {
            let messages: Vec<String> =
                findings.iter().map(|(_, _, m)| m.clone()).collect();
            return CheckResult {
                status: CheckStatus::Error,
                message: messages.join("\n"),
                active_backend: None,
            };
        }

        // Nothing installed at all.
        CheckResult {
            status: CheckStatus::Off,
            message: format!(
                "未安装任何 Reddit 后端。注意：Reddit 没有零配置路径\
                 （匿名 .json 已被封，官方 API 需人工审批），必须用登录态。推荐：\n  \
                 方法一（推荐，零安装）: agent-reach config set reddit_cookie \"<你的 reddit_session>\"\n     \
                 （从浏览器 Cookie 中复制 reddit_session 值即可使用原生 API）\n  \
                 桌面：agent-reach install --channels opencli\n      \
                 （复用 Chrome 登录态，登录过 reddit.com 即可用）\n  \
                 服务器/存量：pipx install '{}'\n      \
                 然后 `rdt login` 或手动写入 Cookie（见 doctor 提示）\n  \
                 中国大陆访问 Reddit 需要代理",
                RDT_GIT_SOURCE
            ),
            active_backend: None,
        }
    }
}

// ── helpers ────────────────────────────────────────────────────────

/// URL-encode a query string.
fn urlencoding(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                result.push(byte as char);
            }
            b' ' => result.push_str("%20"),
            _ => {
                result.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    result
}

/// Extract the "children" array from a Reddit listing response.
/// Reddit listings are shaped like:
///   { "data": { "children": [ { "data": {...} }, ... ] } }
/// We extract just the inner `.data` of each child.
fn extract_children(response: &Value) -> Option<Vec<Value>> {
    let children = response
        .get("data")
        .and_then(|d| d.get("children"))
        .and_then(|c| c.as_array())?;
    let posts: Vec<Value> = children
        .iter()
        .filter_map(|child| child.get("data").cloned())
        .collect();
    Some(posts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_reddit() {
        let ch = RedditChannel::new();
        assert!(ch.can_handle(
            "https://www.reddit.com/r/rust/comments/abc123/"
        ));
        assert!(ch.can_handle("https://reddit.com/r/programming"));
        assert!(ch.can_handle("https://redd.it/abc123"));
        assert!(!ch.can_handle("https://www.youtube.com/watch?v=abc"));
        assert!(!ch.can_handle("https://github.com/user/repo"));
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("rust & go"), "rust%20%26%20go");
        assert_eq!(urlencoding("c++"), "c%2B%2B");
        assert_eq!(urlencoding("abc-123_xyz.~"), "abc-123_xyz.~");
    }

    #[test]
    fn test_extract_children() {
        let data = serde_json::json!({
            "data": {
                "children": [
                    { "data": { "id": "abc", "title": "Hello" } },
                    { "data": { "id": "def", "title": "World" } }
                ]
            }
        });
        let result = extract_children(&data).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["id"], "abc");
        assert_eq!(result[1]["title"], "World");
    }

    #[test]
    fn test_extract_children_empty() {
        let data = serde_json::json!({ "data": { "children": [] } });
        let result = extract_children(&data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_children_missing() {
        let data = serde_json::json!({ "other": true });
        assert!(extract_children(&data).is_none());
    }

    #[test]
    fn test_backends_order() {
        let ch = RedditChannel::new();
        let backends = ch.backends();
        assert_eq!(backends[0], "Reddit API (native)");
        assert_eq!(backends[1], "OpenCLI");
        assert_eq!(backends[2], "rdt-cli");
    }
}

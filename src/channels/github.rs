//! GitHub channel — native GitHub REST API (preferred) with gh CLI fallback.
//!
//! Backends (ordered):
//!   1. GitHub API (native) — zero external deps, uses `github_token` config / `GITHUB_TOKEN` env
//!   2. gh CLI — external Go binary (brew/apt install), fallback for users who prefer it

use serde_json::Value;
use url::Url;

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{probe_command, ProbeStatus};

/// User-Agent string sent with all GitHub API requests.
const USER_AGENT: &str = "agent-reach/1.5.0";

/// GitHub REST API base URL.
const API_BASE: &str = "https://api.github.com";

/// Accept header for the GitHub API (recommended by GitHub).
const ACCEPT_HEADER: &str = "application/vnd.github+json";

pub struct GitHubChannel {
    active_backend: Option<String>,
}

impl GitHubChannel {
    pub fn new() -> Self {
        GitHubChannel {
            active_backend: None,
        }
    }

    // ── token helpers ─────────────────────────────────────────────────

    /// Resolve a GitHub personal access token.
    ///
    /// Checks config key `github_token` first, then env var `GITHUB_TOKEN`.
    fn resolve_token(config: Option<&Config>) -> Option<String> {
        if let Some(cfg) = config {
            if let Some(t) = cfg.get("github_token") {
                let t = t.trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
        std::env::var("GITHUB_TOKEN").ok().map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
    }

    // ── HTTP helpers ──────────────────────────────────────────────────

    /// Build a `ureq` agent pre-configured for the GitHub API.
    fn api_request(token: Option<&str>, path: &str) -> Result<ureq::Response, String> {
        let token = token.unwrap_or("");
        let url = format!("{}{}", API_BASE, path);

        let mut req = ureq::get(&url).set("Accept", ACCEPT_HEADER).set("User-Agent", USER_AGENT);
        if !token.is_empty() {
            req = req.set("Authorization", &format!("Bearer {}", token));
        }
        req.call().map_err(|e| format!("GitHub API request failed: {}", e))
    }

    /// Perform a GET to the GitHub API and parse the body as JSON.
    fn api_get(token: Option<&str>, path: &str) -> Result<Value, String> {
        let resp = Self::api_request(token, path)?;
        let status = resp.status();
        let body = resp
            .into_string()
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        if status >= 200 && status < 300 {
            serde_json::from_str(&body)
                .map_err(|e| format!("Failed to parse JSON response: {}", e))
        } else if status == 404 {
            Err("Not found (404)".to_string())
        } else {
            // Try to extract GitHub error message
            let msg = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| body);
            Err(format!("GitHub API error ({}): {}", status, msg))
        }
    }

    // ── native backend probe ──────────────────────────────────────────

    /// Probe the native GitHub API backend.
    ///
    /// Returns None when neither a token nor other credentials are available.
    /// Returns (status_str, message) when we have something to report.
    fn check_native(&self, config: Option<&Config>) -> Option<(String, String)> {
        let token = Self::resolve_token(config);

        let token_str: Option<&str> = token.as_deref();
        let resp = Self::api_request(token_str, "/rate_limit");

        match resp {
            Ok(r) => {
                let status = r.status();
                let body = r.into_string().unwrap_or_default();

                if status == 200 {
                    // Parse rate_limit to report remaining quota
                    let quota_msg = serde_json::from_str::<Value>(&body)
                        .ok()
                        .and_then(|v| {
                            let core = v.get("resources")?.get("core")?;
                            let remaining = core.get("remaining")?.as_u64()?;
                            let limit = core.get("limit")?.as_u64()?;
                            Some(format!(
                                "（剩余 {} / {} 次请求，{} 后重置）",
                                remaining,
                                limit,
                                core.get("reset")
                                    .and_then(|r| r.as_u64())
                                    .map(|ts| {
                                        let dt = chrono::DateTime::from_timestamp(ts as i64, 0)
                                            .unwrap_or_default();
                                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                                    })
                                    .unwrap_or_else(|| "unknown".to_string())
                            ))
                        })
                        .unwrap_or_default();

                    Some((
                        "ok".to_string(),
                        format!(
                            "GitHub API (native) 完整可用 — 仓库、搜索、Issue、PR 等。{}",
                            quota_msg
                        ),
                    ))
                } else if status == 401 || status == 403 {
                    Some((
                        "warn".to_string(),
                        concat!(
                            "GitHub API (native) — token 无效或无权限。",
                            "请检查配置中的 github_token 或环境变量 GITHUB_TOKEN。\n",
                            "创建 token: https://github.com/settings/tokens"
                        )
                        .to_string(),
                    ))
                } else {
                    Some((
                        "warn".to_string(),
                        format!("GitHub API (native) — 返回 HTTP {}，请稍后重试", status),
                    ))
                }
            }
            Err(e) => {
                if token.is_none() {
                    // No token at all: not an error, just not configured
                    None
                } else {
                    // Token is configured but network failed
                    Some((
                        "warn".to_string(),
                        format!(
                            "GitHub API (native) — 网络请求失败，将回退到 gh CLI。\n{}",
                            e
                        ),
                    ))
                }
            }
        }
    }

    // ── gh CLI backend probe ──────────────────────────────────────────

    /// Probe the gh CLI backend.
    ///
    /// Returns None when gh is not installed.
    fn check_gh_cli(&self) -> Option<(String, String)> {
        let probe = probe_command("gh", &["auth", "status"], 10, 0, Some("gh"));

        match probe.status {
            ProbeStatus::Missing => None,
            ProbeStatus::Broken => Some((
                "error".to_string(),
                concat!(
                    "gh 命令存在但无法执行——安装已损坏。重装即可修复：\n",
                    "  brew reinstall gh\n",
                    "或从 https://cli.github.com 重新安装 gh CLI"
                )
                .to_string(),
            )),
            ProbeStatus::Timeout => Some((
                "warn".to_string(),
                "gh CLI 状态检查超时，运行 gh auth status 查看详情".to_string(),
            )),
            ProbeStatus::Ok => Some((
                "ok".to_string(),
                "gh CLI 完整可用（读取、搜索、Fork、Issue、PR 等）".to_string(),
            )),
            ProbeStatus::Error => Some((
                "warn".to_string(),
                "gh CLI 已安装但未认证。运行 gh auth login 可解锁完整功能".to_string(),
            )),
        }
    }

    // ── public API methods ────────────────────────────────────────────

    /// Get repository information.
    ///
    /// `GET /repos/{owner}/{repo}`
    pub fn get_repo(owner: &str, repo: &str) -> Result<Value, String> {
        let token = Self::resolve_token(None);
        Self::api_get(token.as_deref(), &format!("/repos/{}/{}", owner, repo))
    }

    /// Search code across GitHub.
    ///
    /// `GET /search/code?q={query}`
    pub fn search_code(query: &str) -> Result<Value, String> {
        let token = Self::resolve_token(None);
        let encoded = urlencoding(query);
        Self::api_get(token.as_deref(), &format!("/search/code?q={}", encoded))
    }

    /// Search issues and pull requests across GitHub.
    ///
    /// `GET /search/issues?q={query}`
    pub fn search_issues(query: &str) -> Result<Value, String> {
        let token = Self::resolve_token(None);
        let encoded = urlencoding(query);
        Self::api_get(token.as_deref(), &format!("/search/issues?q={}", encoded))
    }

    /// Get a single issue by owner, repo, and issue number.
    ///
    /// `GET /repos/{owner}/{repo}/issues/{issue_number}`
    pub fn get_issue(owner: &str, repo: &str, issue_number: u64) -> Result<Value, String> {
        let token = Self::resolve_token(None);
        Self::api_get(
            token.as_deref(),
            &format!("/repos/{}/{}/issues/{}", owner, repo, issue_number),
        )
    }
}

impl Default for GitHubChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl Channel for GitHubChannel {
    fn name(&self) -> &str {
        "github"
    }

    fn description(&self) -> &str {
        "GitHub 仓库和代码"
    }

    fn backends(&self) -> &[&str] {
        &["GitHub API (native)", "gh CLI"]
    }

    fn tier(&self) -> u8 {
        1
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => parsed
                .host_str()
                .map(|h| h.to_lowercase().contains("github.com"))
                .unwrap_or(false),
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
        let mut findings: Vec<(String, String, String)> = Vec::new(); // (backend, status, message)

        for backend in self.ordered_backends(config) {
            let result = if backend == "GitHub API (native)" {
                self.check_native(config)
            } else if backend == "gh CLI" {
                self.check_gh_cli()
            } else {
                continue;
            };

            if let Some((status, msg)) = result {
                findings.push((backend, status, msg));
            }
        }

        // First fully-usable (ok) backend wins, then first fixable (warn)
        for wanted in &["ok", "warn"] {
            for (backend, status, message) in &findings {
                if status == *wanted {
                    self.active_backend = Some(backend.clone());
                    let status = CheckStatus::from_str(status);
                    return CheckResult {
                        status,
                        message: message.clone(),
                        active_backend: self.active_backend.clone(),
                    };
                }
            }
        }

        // Only broken/timeout candidates left
        if !findings.is_empty() {
            let messages: Vec<String> = findings.iter().map(|(_, _, m)| m.clone()).collect();
            return CheckResult {
                status: CheckStatus::Error,
                message: messages.join("\n"),
                active_backend: None,
            };
        }

        // Nothing usable found
        CheckResult {
            status: CheckStatus::Warn,
            message: concat!(
                "GitHub 未配置。配置方式（二选一）：\n",
                "  1. 设置 github_token 配置或 GITHUB_TOKEN 环境变量 → 使用原生 API（推荐，零外部依赖）\n",
                "  2. 安装 gh CLI: https://cli.github.com"
            )
            .to_string(),
            active_backend: None,
        }
    }
}

/// Percent-encode a string for use in URL query parameters.
fn urlencoding(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_github() {
        let ch = GitHubChannel::new();
        assert!(ch.can_handle("https://github.com/user/repo"));
        assert!(ch.can_handle("https://www.github.com/user/repo/issues/1"));
        assert!(ch.can_handle("https://github.com/search?q=rust"));
        assert!(!ch.can_handle("https://www.youtube.com/watch?v=abc"));
        assert!(!ch.can_handle("https://twitter.com/user/status/123"));
        assert!(!ch.can_handle("not-a-url"));
    }

    #[test]
    fn test_name_and_tier() {
        let ch = GitHubChannel::new();
        assert_eq!(ch.name(), "github");
        assert_eq!(ch.description(), "GitHub 仓库和代码");
        assert_eq!(ch.tier(), 1);
    }

    #[test]
    fn test_backends_order() {
        let ch = GitHubChannel::new();
        let backends = ch.backends();
        assert_eq!(backends.len(), 2);
        assert_eq!(backends[0], "GitHub API (native)");
        assert_eq!(backends[1], "gh CLI");
    }

    #[test]
    fn test_active_backend_get_set() {
        let mut ch = GitHubChannel::new();
        assert!(ch.active_backend().is_none());
        ch.set_active_backend(Some("GitHub API (native)".to_string()));
        assert_eq!(
            ch.active_backend(),
            Some("GitHub API (native)".to_string())
        );
        ch.set_active_backend(None);
        assert!(ch.active_backend().is_none());
    }

    #[test]
    fn test_urlencoding() {
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("rust+lang"), "rust%2Blang");
        assert_eq!(urlencoding("abc123-_."), "abc123-_.");
        assert_eq!(urlencoding("repo:rust-lang/rust"), "repo%3Arust-lang%2Frust");
    }
}

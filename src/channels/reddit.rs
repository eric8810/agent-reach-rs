//! Reddit — multi-backend: OpenCLI / rdt-cli. Login is mandatory.
//!
//! Honest tiering (live-verified 2026-06): there is NO zero-config path.
//! Anonymous .json endpoints are blocked (403 anti-bot, all variants), and
//! the official API closed self-service registration in 2025-11 (manual
//! approval, individual scripts rarely granted — PRAW is only an option for
//! users who already hold credentials). Every working backend rides a
//! logged-in session: OpenCLI reuses the browser's, rdt-cli imports cookies.

use crate::backends::opencli_status;
use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;

use serde::Deserialize;
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
/// When the venv shebang breaks, the reinstall hint differs from probe's
/// default pipx/uv — rdt needs the specific git source.
const RDT_BROKEN_HINT: &str =
    "rdt 命令存在但无法执行——通常是系统 Python 升级后 venv 解释器丢失。\n\
     PyPI 版本落后，推荐用固定 git 源强制重装：\n\
       pipx install --force 'git+https://github.com/public-clis/rdt-cli.git@5e4fb3720d5c174e976cd425ccc3b879d52cac66'";

/// JSON shape of `rdt status --json` response.
#[derive(Debug, Deserialize)]
struct RdtStatusResponse {
    data: Option<RdtStatusData>,
}

#[derive(Debug, Deserialize)]
struct RdtStatusData {
    authenticated: Option<bool>,
    username: Option<String>,
}

/// Channel for Reddit posts and comments.
pub struct RedditChannel {
    active_backend: Option<String>,
}

impl RedditChannel {
    pub fn new() -> Self {
        RedditChannel { active_backend: None }
    }

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

    /// rdt-cli candidate. None = not installed.
    ///
    /// Don't go through probe_command: `rdt status --json` writes retry
    /// logs to stderr even on success (rc=0), so merged stdout+stderr
    /// would break JSON parsing. Hand-roll subprocess (stdout only),
    /// but classify errors the same way probe does.
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
                // which() found it but exec failed — stale venv shebang
                // (OSError / FileNotFoundError equivalent in Python).
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Some((CheckStatus::Error, RDT_BROKEN_HINT.to_string()));
                }
                return Some((CheckStatus::Error, RDT_BROKEN_HINT.to_string()));
            }
        };

        // Wait with 10s timeout (matching Python's subprocess.run(timeout=10)).
        // child is moved into the thread; we keep the PID so we can kill on timeout.
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
                // Kill the orphaned child by PID.
                // child.id() returns u32 directly.
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
        let stderr = String::from_utf8_lossy(&output.stderr);

        if let Some(code) = output.status.code() {
            if BROKEN_EXIT_CODES.contains(&code) {
                return Some((CheckStatus::Error, RDT_BROKEN_HINT.to_string()));
            }
            if code != 0 {
                let detail: Vec<&str> =
                    stderr.trim().lines().chain(stdout.trim().lines()).collect();
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

        // Process exited normally — rdt itself is alive (logged in or not).
        let data: Option<RdtStatusResponse> = serde_json::from_str(stdout.trim()).ok();

        let info = match data {
            Some(d) => d.data.unwrap_or(RdtStatusData {
                authenticated: None,
                username: None,
            }),
            None => {
                return Some((
                    CheckStatus::Warn,
                    "rdt-cli 可用但状态输出无法解析，运行 `rdt status` 查看登录状态".to_string(),
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
        &["OpenCLI", "rdt-cli"]
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
        // Probe candidates in order; first fully-usable backend wins.
        //
        // Same two-phase logic as Twitter: collect all candidate statuses,
        // first "ok" wins; only if no "ok" does first "warn" win — otherwise
        // an installed-but-not-logged-in rdt-cli would block the fully
        // usable OpenCLI sitting later in the list.
        self.active_backend = None;
        let mut findings: Vec<(String, CheckStatus, String)> = Vec::new();

        for backend in self.ordered_backends(config) {
            let result = match backend.as_str() {
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
            let messages: Vec<String> = findings.iter().map(|(_, _, m)| m.clone()).collect();
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

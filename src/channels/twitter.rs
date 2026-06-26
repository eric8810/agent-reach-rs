//! Twitter/X — multi-backend: twitter-cli / OpenCLI / bird CLI (legacy).
//!
//! Backend order encodes the recommendation:
//! 1. twitter-cli — dedicated Twitter CLI (pip package), full-featured
//! 2. OpenCLI — cross-platform via Chrome browser session, zero per-platform config
//! 3. bird CLI (legacy) — npm package @steipete/bird, maintained fallback

use url::Url;

use crate::backends::{opencli_status, OpenCLIStatus};
use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{npm_reinstall_hint, probe_command, probe_command_with_hint, ProbeStatus};

/// Twitter channel — multi-backend with twitter-cli, OpenCLI, and bird CLI.
pub struct TwitterChannel {
    pub active_backend: Option<String>,
}

impl TwitterChannel {
    pub fn new() -> Self {
        TwitterChannel {
            active_backend: None,
        }
    }

    // ── backend probes ──────────────────────────────────────────────

    /// Probe twitter-cli. None = not installed.
    /// `twitter status` is the health signal: "ok: true" when logged in,
    /// "not_authenticated" with non-zero exit when not logged in.
    fn check_twitter_cli(&self) -> Option<(String, String)> {
        let probe = probe_command("twitter", &["status"], 15, 1, Some("twitter-cli"));

        if probe.status == ProbeStatus::Missing {
            return None;
        }
        if probe.status == ProbeStatus::Broken {
            return Some((
                "error".to_string(),
                format!("twitter-cli 命令存在但无法执行。\n{}", probe.hint),
            ));
        }
        if probe.status == ProbeStatus::Timeout {
            return Some((
                "error".to_string(),
                format!("twitter-cli 健康检查超时（已重试 1 次）。\n{}", probe.hint),
            ));
        }

        let output = &probe.output;
        if output.contains("ok: true") {
            return Some((
                "ok".to_string(),
                concat!(
                    "twitter-cli 完整可用（搜索、读推文、时间线、长文/Article、",
                    "用户查询、Thread）"
                )
                .to_string(),
            ));
        }
        if output.contains("not_authenticated") {
            return Some((
                "warn".to_string(),
                concat!(
                    "twitter-cli 已安装但未认证。设置方式：\n",
                    "  export TWITTER_AUTH_TOKEN=\"xxx\"\n",
                    "  export TWITTER_CT0=\"yyy\"\n",
                    "或确保已在浏览器中登录 x.com"
                )
                .to_string(),
            ));
        }
        Some((
            "warn".to_string(),
            "twitter-cli 已安装但认证检查失败。运行：\n  twitter -v status 查看详细信息"
                .to_string(),
        ))
    }

    /// OpenCLI candidate. None = not installed.
    fn check_opencli(&self) -> Option<(String, String)> {
        let st: OpenCLIStatus = opencli_status(15);
        if !st.installed {
            return None;
        }
        if st.broken {
            return Some(("error".to_string(), st.hint));
        }
        if st.ready() {
            return Some((
                "ok".to_string(),
                concat!(
                    "OpenCLI 可用（复用浏览器登录态）。用法：",
                    "opencli twitter search/article/user-posts -f yaml"
                )
                .to_string(),
            ));
        }
        Some(("warn".to_string(), st.hint))
    }

    /// Legacy bird/birdx candidate. None = neither installed.
    /// Probes "bird" first, then "birdx" as fallback.
    fn check_bird(&self) -> Option<(String, String)> {
        let mut last_failure: Option<(String, String)> = None;

        for cmd in &["bird", "birdx"] {
            let probe = probe_command_with_hint(
                cmd,
                &["check"],
                15,
                1,
                Some("@steipete/bird"),
                Some(npm_reinstall_hint),
            );

            if probe.status == ProbeStatus::Missing {
                continue;
            }
            if probe.status == ProbeStatus::Broken {
                last_failure = Some((
                    "error".to_string(),
                    format!(
                        "{} 命令存在但无法执行（bird 是 npm 包，可用 npm install -g @steipete/bird 重装）。\n{}",
                        cmd, probe.hint
                    ),
                ));
                continue;
            }
            if probe.status == ProbeStatus::Timeout {
                last_failure = Some((
                    "error".to_string(),
                    format!(
                        "{} 健康检查超时（已重试 1 次）。\n{}",
                        cmd, probe.hint
                    ),
                ));
                continue;
            }

            let output = &probe.output;
            if probe.ok() {
                return Some((
                    "ok".to_string(),
                    "bird CLI 可用（读取、搜索推文，含长文/X Article）".to_string(),
                ));
            }
            if output.contains("Missing credentials") || output.to_lowercase().contains("missing") {
                return Some((
                    "warn".to_string(),
                    concat!(
                        "bird CLI 已安装但未配置认证。设置环境变量：\n",
                        "  export AUTH_TOKEN=\"xxx\"\n",
                        "  export CT0=\"yyy\""
                    )
                    .to_string(),
                ));
            }
            return Some((
                "warn".to_string(),
                "bird CLI 已安装但认证检查失败。".to_string(),
            ));
        }

        last_failure
    }
}

impl Default for TwitterChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl Channel for TwitterChannel {
    fn name(&self) -> &str {
        "twitter"
    }

    fn description(&self) -> &str {
        "Twitter/X 推文"
    }

    fn backends(&self) -> &[&str] {
        &["twitter-cli", "OpenCLI", "bird CLI (legacy)"]
    }

    fn tier(&self) -> u8 {
        1
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_lowercase();
                host.contains("x.com") || host.contains("twitter.com")
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
        let mut findings: Vec<(String, String, String)> = Vec::new(); // (backend, status, message)

        for backend in self.ordered_backends(config) {
            let result = if backend == "twitter-cli" {
                self.check_twitter_cli()
            } else if backend == "OpenCLI" {
                self.check_opencli()
            } else if backend == "bird CLI (legacy)" {
                self.check_bird()
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

        // Nothing installed at all
        CheckResult {
            status: CheckStatus::Warn,
            message: concat!(
                "Twitter CLI 未安装。安装方式：\n",
                "  pipx install twitter-cli\n",
                "或：\n",
                "  uv tool install twitter-cli"
            )
            .to_string(),
            active_backend: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_twitter() {
        let ch = TwitterChannel::new();
        assert!(ch.can_handle("https://x.com/user/status/123"));
        assert!(ch.can_handle("https://twitter.com/user/status/456"));
        assert!(ch.can_handle("https://www.x.com/search?q=rust"));
        assert!(!ch.can_handle("https://www.youtube.com/watch?v=abc"));
        assert!(!ch.can_handle("https://github.com/user/repo"));
        assert!(!ch.can_handle("not-a-url"));
    }

    #[test]
    fn test_name_and_tier() {
        let ch = TwitterChannel::new();
        assert_eq!(ch.name(), "twitter");
        assert_eq!(ch.description(), "Twitter/X 推文");
        assert_eq!(ch.tier(), 1);
    }

    #[test]
    fn test_backends_order() {
        let ch = TwitterChannel::new();
        let backends = ch.backends();
        assert_eq!(backends.len(), 3);
        assert_eq!(backends[0], "twitter-cli");
        assert_eq!(backends[1], "OpenCLI");
        assert_eq!(backends[2], "bird CLI (legacy)");
    }

    #[test]
    fn test_active_backend_get_set() {
        let mut ch = TwitterChannel::new();
        assert!(ch.active_backend().is_none());
        ch.set_active_backend(Some("twitter-cli".to_string()));
        assert_eq!(ch.active_backend(), Some("twitter-cli".to_string()));
        ch.set_active_backend(None);
        assert!(ch.active_backend().is_none());
    }
}

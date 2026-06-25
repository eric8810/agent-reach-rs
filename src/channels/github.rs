//! GitHub channel — check if gh CLI is available.

use url::Url;

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{probe_command, ProbeStatus};

pub struct GitHubChannel {
    active_backend: Option<String>,
}

impl GitHubChannel {
    pub fn new() -> Self {
        GitHubChannel {
            active_backend: None,
        }
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
        &["gh CLI"]
    }

    fn tier(&self) -> u8 {
        0
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

    fn check(&mut self, _config: Option<&Config>) -> CheckResult {
        // Run gh auth status for real. Note: rc!=0 when not logged in is
        // expected business state (warn), not an error.
        let probe = probe_command("gh", &["auth", "status"], 10, 0, Some("gh"));

        match probe.status {
            ProbeStatus::Missing => {
                self.active_backend = None;
                CheckResult {
                    status: CheckStatus::Warn,
                    message: "gh CLI 未安装。安装：https://cli.github.com".to_string(),
                    active_backend: None,
                }
            }
            ProbeStatus::Broken => {
                // gh is a binary install (brew/official package), not a pip package —
                // use brew/system-specific reinstall advice.
                self.active_backend = None;
                CheckResult {
                    status: CheckStatus::Error,
                    message: concat!(
                        "gh 命令存在但无法执行——安装已损坏。重装即可修复：\n",
                        "  brew reinstall gh\n",
                        "或从 https://cli.github.com 重新安装 gh CLI"
                    )
                    .to_string(),
                    active_backend: None,
                }
            }
            ProbeStatus::Timeout => {
                // gh binary launched (tool is alive), just the status check timed out.
                self.active_backend = Some("gh CLI".to_string());
                CheckResult {
                    status: CheckStatus::Warn,
                    message: "gh CLI 状态检查超时，运行 gh auth status 查看详情".to_string(),
                    active_backend: Some("gh CLI".to_string()),
                }
            }
            ProbeStatus::Ok => {
                self.active_backend = Some("gh CLI".to_string());
                CheckResult {
                    status: CheckStatus::Ok,
                    message: "完整可用（读取、搜索、Fork、Issue、PR 等）".to_string(),
                    active_backend: Some("gh CLI".to_string()),
                }
            }
            ProbeStatus::Error => {
                // rc != 0: gh is alive but not authenticated
                // (gh auth status normal business state)
                self.active_backend = Some("gh CLI".to_string());
                CheckResult {
                    status: CheckStatus::Warn,
                    message: "gh CLI 已安装但未认证。运行 gh auth login 可解锁完整功能"
                        .to_string(),
                    active_backend: Some("gh CLI".to_string()),
                }
            }
        }
    }
}

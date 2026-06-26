//! Exa Search — check if mcporter + Exa MCP is available.

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{npm_reinstall_hint, probe_command_with_hint, ProbeStatus};

/// Channel for Exa semantic web search via mcporter MCP.
pub struct ExaSearchChannel {
    active_backend: Option<String>,
}

impl ExaSearchChannel {
    pub fn new() -> Self {
        ExaSearchChannel {
            active_backend: None,
        }
    }
}

impl Channel for ExaSearchChannel {
    fn name(&self) -> &str {
        "exa_search"
    }

    fn description(&self) -> &str {
        "全网语义搜索"
    }

    fn backends(&self) -> &[&str] {
        &["Exa via mcporter"]
    }

    fn tier(&self) -> u8 {
        0
    }

    fn can_handle(&self, _url: &str) -> bool {
        false // Search-only channel
    }

    fn active_backend(&self) -> Option<String> {
        self.active_backend.clone()
    }

    fn set_active_backend(&mut self, backend: Option<String>) {
        self.active_backend = backend;
    }

    fn check(&mut self, _config: Option<&Config>) -> CheckResult {
        self.active_backend = None;

        let probe = probe_command_with_hint(
            "mcporter",
            &["config", "list"],
            10,
            0,
            Some("mcporter"),
            Some(npm_reinstall_hint),
        );

        match probe.status {
            ProbeStatus::Missing => CheckResult {
                status: CheckStatus::Off,
                message: concat!(
                    "需要 mcporter + Exa MCP。安装：\n",
                    "  npm install -g mcporter\n",
                    "  mcporter config add exa https://mcp.exa.ai/mcp",
                )
                .to_string(),
                active_backend: None,
            },
            ProbeStatus::Broken => CheckResult {
                status: CheckStatus::Error,
                message: format!(
                    "mcporter 无法执行（node 环境损坏），重装：\n  npm install -g mcporter{}",
                    if !probe.hint.is_empty() {
                        format!("\n{}", probe.hint)
                    } else {
                        String::new()
                    }
                ),
                active_backend: None,
            },
            ProbeStatus::Timeout | ProbeStatus::Error => {
                let detail = if probe.hint.is_empty() {
                    if probe.output.is_empty() {
                        probe.status.as_str().to_string()
                    } else {
                        probe.output.clone()
                    }
                } else {
                    probe.hint.clone()
                };
                CheckResult {
                    status: CheckStatus::Error,
                    message: format!("mcporter 执行异常：{}", detail),
                    active_backend: None,
                }
            }
            ProbeStatus::Ok => {
                if probe.output.to_lowercase().contains("exa") {
                    self.active_backend = Some(self.backends()[0].to_string());
                    CheckResult {
                        status: CheckStatus::Ok,
                        message: "全网语义搜索可用（免费，无需 API Key）".to_string(),
                        active_backend: self.active_backend.clone(),
                    }
                } else {
                    CheckResult {
                        status: CheckStatus::Off,
                        message: concat!(
                            "mcporter 已装但 Exa 未配置。运行：\n",
                            "  mcporter config add exa https://mcp.exa.ai/mcp",
                        )
                        .to_string(),
                        active_backend: None,
                    }
                }
            }
        }
    }
}

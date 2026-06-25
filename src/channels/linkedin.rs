//! LinkedIn channel — check mcporter and linkedin-scraper-mcp availability.
//!
//! LinkedIn requires the linkedin-scraper-mcp MCP server registered with mcporter.
//! Fallback backend: Jina Reader provides basic content access.

use url::Url;

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{npm_reinstall_hint, probe_command_with_hint, ProbeStatus};

pub struct LinkedInChannel {
    active_backend: Option<String>,
}

impl LinkedInChannel {
    pub fn new() -> Self {
        LinkedInChannel {
            active_backend: None,
        }
    }
}

impl Default for LinkedInChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl Channel for LinkedInChannel {
    fn name(&self) -> &str {
        "linkedin"
    }

    fn description(&self) -> &str {
        "LinkedIn 职业社交"
    }

    fn backends(&self) -> &[&str] {
        &["linkedin-scraper-mcp", "Jina Reader"]
    }

    fn tier(&self) -> u8 {
        2
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => parsed
                .host_str()
                .map(|h| h.to_lowercase().contains("linkedin.com"))
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
        self.active_backend = None;

        let probe = probe_command_with_hint(
            "mcporter",
            &["config", "list"],
            10,
            0,
            Some("mcporter"),
            Some(npm_reinstall_hint),
        );

        if probe.status == ProbeStatus::Missing {
            return CheckResult {
                status: CheckStatus::Off,
                message: concat!(
                    "基本内容可通过 Jina Reader 读取。完整功能需要：\n",
                    "  pip install linkedin-scraper-mcp\n",
                    "  mcporter config add linkedin http://localhost:3000/mcp\n",
                    "  详见 https://github.com/stickerdaniel/linkedin-mcp-server"
                )
                .to_string(),
                active_backend: None,
            };
        }

        if probe.status == ProbeStatus::Broken {
            return CheckResult {
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
            };
        }

        if !probe.ok() {
            let detail = if !probe.hint.is_empty() {
                probe.hint
            } else if !probe.output.is_empty() {
                probe.output
            } else {
                probe.status.as_str().to_string()
            };
            return CheckResult {
                status: CheckStatus::Error,
                message: format!("mcporter 执行异常：{}", detail),
                active_backend: None,
            };
        }

        if probe.output.to_lowercase().contains("linkedin") {
            self.active_backend = Some("linkedin-scraper-mcp".to_string());
            return CheckResult {
                status: CheckStatus::Ok,
                message: "完整可用（Profile、公司、职位搜索）".to_string(),
                active_backend: self.active_backend.clone(),
            };
        }

        CheckResult {
            status: CheckStatus::Off,
            message: concat!(
                "mcporter 已装但 LinkedIn MCP 未配置。运行：\n",
                "  pip install linkedin-scraper-mcp\n",
                "  mcporter config add linkedin http://localhost:3000/mcp"
            )
            .to_string(),
            active_backend: None,
        }
    }
}

//! LinkedIn channel — multi-backend: native MCP / mcporter-linkedin / Jina Reader.
//!
//! Backend order:
//!   1. LinkedIn MCP (native)   — directly call linkedin-scraper-mcp at localhost:3000/mcp
//!   2. linkedin-scraper-mcp    — legacy mcporter MCP proxy
//!   3. Jina Reader             — basic public page content access (no auth needed)

use std::time::Duration;
use url::Url;

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{npm_reinstall_hint, probe_command_with_hint, ProbeStatus};

/// LinkedIn MCP server URL (linkedin-scraper-mcp running locally).
const LINKEDIN_MCP_URL: &str = "http://localhost:3000/mcp";

pub struct LinkedInChannel {
    active_backend: Option<String>,
}

impl LinkedInChannel {
    pub fn new() -> Self {
        LinkedInChannel {
            active_backend: None,
        }
    }

    // ── LinkedIn MCP (native) backend ─────────────────────────────────

    /// Probe the native LinkedIn MCP endpoint at localhost:3000/mcp.
    ///
    /// Sends a GET; if reachable the native backend is usable.
    /// Returns None on transport errors so we fall through to fallbacks.
    fn check_native() -> Option<(CheckStatus, String)> {
        match ureq::get(LINKEDIN_MCP_URL)
            .timeout(Duration::from_secs(5))
            .call()
        {
            Ok(resp) => {
                let status_code = resp.status();
                Some((
                    CheckStatus::Ok,
                    format!(
                        "LinkedIn MCP (native) 可用 — 直接连接 linkedin-scraper-mcp (HTTP {})，零外部依赖",
                        status_code
                    ),
                ))
            }
            Err(ureq::Error::Status(code, _)) => {
                // Server responded but with an error — still reachable
                Some((
                    CheckStatus::Warn,
                    format!(
                        "LinkedIn MCP (native) — 端点返回 HTTP {}。请确认 linkedin-scraper-mcp 运行正常：\n  pip install linkedin-scraper-mcp\n  详见 https://github.com/stickerdaniel/linkedin-mcp-server",
                        code
                    ),
                ))
            }
            Err(ureq::Error::Transport(e)) => {
                let msg = e.to_string();
                if msg.contains("timeout") || msg.contains("timed out") {
                    Some((
                        CheckStatus::Error,
                        "LinkedIn MCP (native) 连接超时，linkedin-scraper-mcp 可能未运行。启动：\n  linkedin-scraper-mcp\n  然后检查 http://localhost:3000/mcp".to_string(),
                    ))
                } else {
                    // Connection refused, DNS failure, etc. — let fallbacks try.
                    None
                }
            }
        }
    }

    // ── linkedin-scraper-mcp (mcporter) backend ───────────────────────

    /// Probe the mcporter + linkedin-scraper-mcp backend (legacy).
    fn check_mcporter() -> Option<(CheckStatus, String)> {
        let probe = probe_command_with_hint(
            "mcporter",
            &["config", "list"],
            10,
            0,
            Some("mcporter"),
            Some(npm_reinstall_hint),
        );

        if probe.status == ProbeStatus::Missing {
            return Some((
                CheckStatus::Off,
                concat!(
                    "基本内容可通过 Jina Reader 读取。完整功能需要：\n",
                    "  pip install linkedin-scraper-mcp\n",
                    "  mcporter config add linkedin http://localhost:3000/mcp\n",
                    "  详见 https://github.com/stickerdaniel/linkedin-mcp-server"
                )
                .to_string(),
            ));
        }

        if probe.status == ProbeStatus::Broken {
            return Some((
                CheckStatus::Error,
                format!(
                    "mcporter 无法执行（node 环境损坏），重装：\n  npm install -g mcporter{}",
                    if !probe.hint.is_empty() {
                        format!("\n{}", probe.hint)
                    } else {
                        String::new()
                    }
                ),
            ));
        }

        if !probe.ok() {
            let detail = if !probe.hint.is_empty() {
                probe.hint
            } else if !probe.output.is_empty() {
                probe.output
            } else {
                probe.status.as_str().to_string()
            };
            return Some((
                CheckStatus::Error,
                format!("mcporter 执行异常：{}", detail),
            ));
        }

        if probe.output.to_lowercase().contains("linkedin") {
            Some((
                CheckStatus::Ok,
                "完整可用（Profile、公司、职位搜索）".to_string(),
            ))
        } else {
            Some((
                CheckStatus::Off,
                concat!(
                    "mcporter 已装但 LinkedIn MCP 未配置。运行：\n",
                    "  pip install linkedin-scraper-mcp\n",
                    "  mcporter config add linkedin http://localhost:3000/mcp"
                )
                .to_string(),
            ))
        }
    }

    // ── Jina Reader backend ───────────────────────────────────────────

    /// Jina Reader is always available as fallback for basic content access.
    fn check_jina() -> Option<(CheckStatus, String)> {
        // Probe Jina Reader endpoint
        match ureq::get("https://r.jina.ai/http://example.com")
            .set("Accept", "text/plain")
            .timeout(Duration::from_secs(10))
            .call()
        {
            Ok(_resp) => Some((
                CheckStatus::Ok,
                "Jina Reader 可用 — 可读取 LinkedIn 公开页面内容（无需认证）".to_string(),
            )),
            Err(ureq::Error::Status(code, _)) => {
                // Jina responded — still usable
                Some((
                    CheckStatus::Ok,
                    format!(
                        "Jina Reader 可用 (HTTP {}) — 可读取 LinkedIn 公开页面内容（无需认证）",
                        code
                    ),
                ))
            }
            Err(ureq::Error::Transport(e)) => {
                let msg = e.to_string();
                if msg.contains("timeout") || msg.contains("timed out") {
                    Some((
                        CheckStatus::Warn,
                        "Jina Reader 连接超时，可能需要代理访问 https://r.jina.ai".to_string(),
                    ))
                } else {
                    // Network down — but Jina is still the last resort
                    Some((
                        CheckStatus::Warn,
                        format!(
                            "Jina Reader 暂时不可达 ({})，网络恢复后可用。",
                            e
                        ),
                    ))
                }
            }
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
        &["LinkedIn MCP (native)", "linkedin-scraper-mcp", "Jina Reader"]
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

    fn check(&mut self, config: Option<&Config>) -> CheckResult {
        self.active_backend = None;
        let mut findings: Vec<(String, CheckStatus, String)> = Vec::new();

        for backend in self.ordered_backends(config) {
            let result = match backend.as_str() {
                "LinkedIn MCP (native)" => Self::check_native(),
                "linkedin-scraper-mcp" => Self::check_mcporter(),
                "Jina Reader" => Self::check_jina(),
                _ => continue,
            };

            if let Some((status, msg)) = result {
                findings.push((backend, status, msg));
            }
        }

        // First fully-usable (ok) backend wins, then first fixable (warn)
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

        // Only error/off candidates left
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
            status: CheckStatus::Off,
            message: concat!(
                "LinkedIn 未配置。配置方式：\n",
                "  1. 运行 linkedin-scraper-mcp（推荐）：pip install linkedin-scraper-mcp && linkedin-scraper-mcp\n",
                "  2. 或通过 mcporter 代理：mcporter config add linkedin http://localhost:3000/mcp\n",
                "  3. Jina Reader 可读取公开页面内容（无需安装，自动回退）\n",
                "  详见 https://github.com/stickerdaniel/linkedin-mcp-server"
            )
            .to_string(),
            active_backend: None,
        }
    }
}

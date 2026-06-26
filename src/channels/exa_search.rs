//! Exa Search — multi-backend: native Exa MCP API / mcporter fallback.
//!
//! Backend order:
//!   1. Exa API (native) — directly call the Exa MCP HTTP endpoint (ureq)
//!   2. Exa via mcporter  — legacy mcporter MCP proxy (npm dependency)

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{npm_reinstall_hint, probe_command_with_hint, ProbeStatus};
use std::time::Duration;

/// Exa MCP endpoint URL.
const EXA_MCP_URL: &str = "https://mcp.exa.ai/mcp";

/// Channel for Exa semantic web search via native MCP or mcporter MCP proxy.
pub struct ExaSearchChannel {
    active_backend: Option<String>,
}

impl ExaSearchChannel {
    pub fn new() -> Self {
        ExaSearchChannel {
            active_backend: None,
        }
    }

    // ── Exa API (native) backend ──────────────────────────────────────

    /// Probe the native Exa MCP HTTP endpoint.
    ///
    /// Sends a GET to `https://mcp.exa.ai/mcp`. If reachable (any HTTP response),
    /// the native backend is usable. Returns None on transport errors so we
    /// fall through to mcporter.
    fn check_native() -> Option<(CheckStatus, String)> {
        match ureq::get(EXA_MCP_URL)
            .timeout(Duration::from_secs(10))
            .call()
        {
            Ok(resp) => {
                let status_code = resp.status();
                Some((
                    CheckStatus::Ok,
                    format!(
                        "Exa API (native) 可用 — 直接连接 Exa MCP 端点 (HTTP {})，零外部依赖",
                        status_code
                    ),
                ))
            }
            Err(ureq::Error::Status(code, _)) => {
                // Server responded but with an error code — still reachable
                Some((
                    CheckStatus::Warn,
                    format!(
                        "Exa API (native) — MCP 端点返回 HTTP {}，可能需 API Key。\n  配置: agent-reach config set exa_api_key <key>",
                        code
                    ),
                ))
            }
            Err(ureq::Error::Transport(e)) => {
                let msg = e.to_string();
                // Network-level failure: unreachable, timeout, DNS, TLS, etc.
                if msg.contains("timeout") || msg.contains("timed out") {
                    Some((
                        CheckStatus::Error,
                        "Exa API (native) 连接超时，将回退到 mcporter。".to_string(),
                    ))
                } else {
                    // Connection refused, DNS failure, etc. — let fallbacks try.
                    None
                }
            }
        }
    }

    // ── mcporter backend ──────────────────────────────────────────────

    /// Probe the mcporter + Exa MCP backend (legacy).
    fn check_mcporter() -> Option<(CheckStatus, String)> {
        let probe = probe_command_with_hint(
            "mcporter",
            &["config", "list"],
            10,
            0,
            Some("mcporter"),
            Some(npm_reinstall_hint),
        );

        match probe.status {
            ProbeStatus::Missing => Some((
                CheckStatus::Off,
                concat!(
                    "需要 mcporter + Exa MCP。安装：\n",
                    "  npm install -g mcporter\n",
                    "  mcporter config add exa https://mcp.exa.ai/mcp"
                )
                .to_string(),
            )),
            ProbeStatus::Broken => Some((
                CheckStatus::Error,
                format!(
                    "mcporter 无法执行（node 环境损坏），重装：\n  npm install -g mcporter{}",
                    if !probe.hint.is_empty() {
                        format!("\n{}", probe.hint)
                    } else {
                        String::new()
                    }
                ),
            )),
            ProbeStatus::Timeout | ProbeStatus::Error => {
                let detail = if !probe.hint.is_empty() {
                    probe.hint
                } else if !probe.output.is_empty() {
                    probe.output
                } else {
                    probe.status.as_str().to_string()
                };
                Some((
                    CheckStatus::Error,
                    format!("mcporter 执行异常：{}", detail),
                ))
            }
            ProbeStatus::Ok => {
                if probe.output.to_lowercase().contains("exa") {
                    Some((
                        CheckStatus::Ok,
                        "全网语义搜索可用（免费，无需 API Key）".to_string(),
                    ))
                } else {
                    Some((
                        CheckStatus::Off,
                        concat!(
                            "mcporter 已装但 Exa 未配置。运行：\n",
                            "  mcporter config add exa https://mcp.exa.ai/mcp"
                        )
                        .to_string(),
                    ))
                }
            }
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
        &["Exa API (native)", "Exa via mcporter"]
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

    fn check(&mut self, config: Option<&Config>) -> CheckResult {
        self.active_backend = None;
        let mut findings: Vec<(String, CheckStatus, String)> = Vec::new();

        for backend in self.ordered_backends(config) {
            let result = match backend.as_str() {
                "Exa API (native)" => Self::check_native(),
                "Exa via mcporter" => Self::check_mcporter(),
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
                "Exa 语义搜索未配置。配置方式：\n",
                "  1. 直接连接 Exa MCP（推荐，零外部依赖）— 端点: https://mcp.exa.ai/mcp\n",
                "  2. 安装 mcporter: npm install -g mcporter"
            )
            .to_string(),
            active_backend: None,
        }
    }
}

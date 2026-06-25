//! Environment health checker — powered by channels.
//!
//! Each channel knows how to check itself. Doctor just collects the results.

use crate::config::Config;
use crate::channels::get_all_channels;

use std::collections::HashMap;

/// Result from a single channel check.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ChannelResult {
    pub status: String,       // "ok" | "warn" | "off" | "error"
    pub name: String,         // display name (description)
    pub message: String,      // human-readable message
    pub tier: u8,             // 0, 1, or 2
    pub backends: Vec<String>,
    pub active_backend: Option<String>,
}

/// Check all channels and return status map.
///
/// A single misbehaving channel must never take the whole report down,
/// so per-channel panics degrade to status="error".
pub fn check_all(config: &Config) -> HashMap<String, ChannelResult> {
    let mut results = HashMap::new();
    let mut channels = get_all_channels();

    for ch in &mut channels {
        let name = ch.name().to_string();
        let description = ch.description().to_string();
        let tier = ch.tier();
        let backends = ch.backends().iter().map(|s| s.to_string()).collect();

        let (status, message, active_backend) = match std::panic::catch_unwind(
            std::panic::AssertUnwindSafe(|| ch.check(Some(config)))
        ) {
            Ok(cr) => (cr.status.as_str().to_string(), cr.message, cr.active_backend),
            Err(e) => {
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    format!("体检异常：{}", s)
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    format!("体检异常：{}", s)
                } else {
                    "体检异常：未知错误".to_string()
                };
                ("error".to_string(), msg, None)
            }
        };

        results.insert(
            name,
            ChannelResult {
                status,
                name: description,
                message,
                tier,
                backends,
                active_backend,
            },
        );
    }

    results
}

/// Format results as a readable text report.
pub fn format_report(results: &HashMap<String, ChannelResult>) -> String {
    let mut lines = Vec::new();

    lines.push("Agent Reach 状态".to_string());
    lines.push("=".repeat(40));
    lines.push("图例：✅ 可用  [!] 已装但需配置/登录  [X] 未安装".to_string());
    lines.push(String::new());

    let ok_count = results.values().filter(|r| r.status == "ok").count();
    let total = results.len();

    // Tier 0 — zero config
    lines.push("✅ 装好即用：".to_string());
    for (_key, r) in results.iter() {
        if r.tier == 0 {
            let icon = match r.status.as_str() {
                "ok" => "  ✅",
                "warn" => "  [!]",
                _ => "  [X]",
            };
            let mut line = format!("{} {} — {}", icon, r.name, r.message);
            if let Some(active) = &r.active_backend {
                if r.backends.len() > 1 {
                    line.push_str(&format!("（当前后端：{}）", active));
                }
            }
            lines.push(line);
        }
    }

    // Tier 1 — needs free key / login
    let tier1: Vec<_> = results.iter().filter(|(_, r)| r.tier == 1).collect();
    let tier1_active: Vec<_> = tier1.iter().filter(|(_, r)| r.status == "ok").collect();
    let tier1_inactive: Vec<_> = tier1.iter().filter(|(_, r)| r.status != "ok").collect();

    if !tier1_active.is_empty() {
        lines.push(String::new());
        lines.push("可选渠道（已安装）：".to_string());
        for (_, r) in &tier1_active {
            lines.push(format!("  ✅ {} — {}", r.name, r.message));
        }
    }

    // Tier 2 — optional complex setup
    let tier2: Vec<_> = results.iter().filter(|(_, r)| r.tier == 2).collect();
    let tier2_active: Vec<_> = tier2.iter().filter(|(_, r)| r.status == "ok").collect();
    let tier2_inactive: Vec<_> = tier2.iter().filter(|(_, r)| r.status != "ok").collect();

    if !tier2_active.is_empty() {
        if tier1_active.is_empty() {
            lines.push(String::new());
            lines.push("可选渠道（已安装）：".to_string());
        }
        for (_, r) in &tier2_active {
            lines.push(format!("  ✅ {} — {}", r.name, r.message));
        }
    }

    lines.push(String::new());
    let status_color = if ok_count == total { "green" } else if ok_count > 0 { "yellow" } else { "red" };
    lines.push(format!("状态：[{}]{}/{}[/{}] 个渠道可用", status_color, ok_count, total, status_color));

    // Summarize inactive optional channels
    let all_inactive: Vec<_> = tier1_inactive.iter().chain(tier2_inactive.iter()).collect();
    if !all_inactive.is_empty() {
        let names: Vec<_> = all_inactive.iter().map(|(_, r)| r.name.as_str()).collect();
        lines.push(format!(
            "还有 {} 个可选渠道可以解锁（{}），告诉你的 Agent「帮我装 XXX」即可",
            names.len(),
            names.join("、")
        ));
    }

    // Security check: config file permissions (Unix only)
    #[cfg(unix)]
    {
        let config_path = Config::config_dir().join("config.yaml");
        if config_path.exists() {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = config_path.metadata() {
                let mode = metadata.permissions().mode();
                if mode & 0o044 != 0 {
                    // group or other read
                    lines.push(String::new());
                    lines.push("[!]  安全提示：config.yaml 权限过宽（其他用户可读）".to_string());
                    lines.push("   修复：chmod 600 ~/.agent-reach/config.yaml".to_string());
                }
            }
        }
    }

    lines.join("\n")
}

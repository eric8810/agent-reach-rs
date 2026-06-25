//! Bilibili — multi-backend: bili-cli / OpenCLI / search API.
//!
//! yt-dlp was REMOVED from this channel (live-verified 2026-06): bilibili's
//! risk control 412-blocks yt-dlp's requests in every configuration we
//! tried — latest version, direct, proxied, with warmed cookies — while
//! bili-cli keeps working (search/hot/video detail without login) and
//! OpenCLI covers subtitles through the browser session.

use crate::backends::opencli_status;
use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{probe_command, ProbeStatus};
use serde_json::Value;
use url::Url;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";
const TIMEOUT: u64 = 10;
const SEARCH_API: &str =
    "https://api.bilibili.com/x/web-interface/search/all/v2?keyword=test&page=1";

/// Return true if the Bilibili search API responds with code 0.
fn search_api_ok() -> bool {
    let resp = ureq::get(SEARCH_API)
        .set("User-Agent", UA)
        .timeout(std::time::Duration::from_secs(TIMEOUT));
    match resp.call() {
        Ok(r) => {
            if let Ok(json) = r.into_json::<Value>() {
                json.get("code").and_then(|v| v.as_i64()) == Some(0)
            } else {
                false
            }
        }
        Err(_) => false,
    }
}

/// Bilibili channel — extract video info, subtitles, search.
pub struct BilibiliChannel {
    pub active_backend: Option<String>,
}

impl BilibiliChannel {
    pub fn new() -> Self {
        BilibiliChannel {
            active_backend: None,
        }
    }

    /// bili-cli candidate. Returns None if not installed.
    fn check_bili_cli(&self) -> Option<(CheckStatus, String)> {
        let probe = probe_command("bili", &["--version"], TIMEOUT, 0, Some("bilibili-cli"));

        match probe.status {
            ProbeStatus::Missing => None,
            ProbeStatus::Broken => Some((
                CheckStatus::Error,
                format!("bili 命令存在但无法执行\n{}", probe.hint),
            )),
            _ if !probe.ok() => Some((
                CheckStatus::Warn,
                format!(
                    "bili-cli 探测失败（{}），运行 `bili status` 查看详情",
                    probe.status.as_str()
                ),
            )),
            _ => Some((
                CheckStatus::Ok,
                "bili-cli 可用（搜索/热门/排行/视频详情/音频，无需登录；字幕需 OpenCLI。上游 2026-03 起停更）".to_string(),
            )),
        }
    }

    /// OpenCLI candidate. Returns None if not installed.
    fn check_opencli(&self) -> Option<(CheckStatus, String)> {
        let st = opencli_status(TIMEOUT);
        if !st.installed {
            return None;
        }
        if st.broken {
            return Some((CheckStatus::Error, st.hint));
        }
        if st.ready() {
            return Some((
                CheckStatus::Ok,
                "OpenCLI 可用（复用浏览器登录态）。用法：opencli bilibili search/video/subtitle/ranking -f yaml"
                    .to_string(),
            ));
        }
        Some((CheckStatus::Warn, st.hint))
    }

    /// Zero-dependency search API fallback. Returns None if unreachable.
    fn check_search_api(&self) -> Option<(CheckStatus, String)> {
        if !search_api_ok() {
            return None;
        }
        Some((
            CheckStatus::Ok,
            "B站搜索 API 可达（仅搜索，curl 直连）。完整功能建议安装 bili-cli：pipx install bilibili-cli"
                .to_string(),
        ))
    }
}

impl Channel for BilibiliChannel {
    fn name(&self) -> &str {
        "bilibili"
    }

    fn description(&self) -> &str {
        "B站视频、字幕和搜索"
    }

    fn backends(&self) -> &[&str] {
        &["bili-cli", "OpenCLI", "B站搜索 API"]
    }

    fn tier(&self) -> u8 {
        1
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_lowercase();
                host.contains("bilibili.com") || host.contains("b23.tv")
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

        // Probe candidates in order; first fully-usable backend wins.
        let candidates = self.ordered_backends(config);
        let mut findings: Vec<(&str, CheckStatus, String)> = Vec::new();

        for backend in &candidates {
            let backend_str = backend.as_str();
            let result = match backend_str {
                "bili-cli" => self.check_bili_cli(),
                "OpenCLI" => self.check_opencli(),
                _ => self.check_search_api(),
            };
            if let Some((status, message)) = result {
                findings.push((backend_str, status, message));
            }
        }

        // Collect broken notes from error findings — surfaced even on success.
        let broken_notes: Vec<&str> = findings
            .iter()
            .filter(|(_, s, _)| *s == CheckStatus::Error)
            .map(|(_, _, m)| m.as_str())
            .collect();

        // First Ok wins, else first Warn, else Error, else Off.
        for wanted in [CheckStatus::Ok, CheckStatus::Warn] {
            for (backend, status, message) in &findings {
                if *status == wanted {
                    self.active_backend = Some(backend.to_string());
                    let mut msg = message.clone();
                    if !broken_notes.is_empty() {
                        msg.push_str("\n[备选后端异常] ");
                        msg.push_str(&broken_notes.join("；"));
                    }
                    return CheckResult {
                        status: *status,
                        message: msg,
                        active_backend: self.active_backend.clone(),
                    };
                }
            }
        }

        if !findings.is_empty() {
            let combined: Vec<String> = findings.iter().map(|(_, _, m)| m.clone()).collect();
            return CheckResult {
                status: CheckStatus::Error,
                message: combined.join("\n"),
                active_backend: None,
            };
        }

        CheckResult {
            status: CheckStatus::Off,
            message: "没有可用的 B站后端（搜索 API 也不可达，可能是网络问题）。推荐：\n  pipx install bilibili-cli（搜索/热门/视频详情，无需登录）\n  或桌面装 OpenCLI（额外解锁字幕）：agent-reach install --channels opencli".to_string(),
            active_backend: None,
        }
    }
}

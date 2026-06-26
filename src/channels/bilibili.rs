//! Bilibili — multi-backend: native API / bili-cli / OpenCLI.
//!
//! yt-dlp was REMOVED from this channel (live-verified 2026-06): bilibili's
//! risk control 412-blocks yt-dlp's requests in every configuration we
//! tried — latest version, direct, proxied, with warmed cookies — while
//! bili-cli keeps working (search/hot/video detail without login) and
//! OpenCLI covers subtitles through the browser session.
//!
//! Preferred backend: "B站 API (native)" — zero-dependency public Bilibili API
//! covering search, video detail, rankings, user videos, and user info.

use crate::backends::opencli_status;
use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{probe_command, ProbeStatus};
use serde_json::Value;
use url::Url;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const REFERER: &str = "https://www.bilibili.com";
const TIMEOUT: u64 = 10;

// ---------------------------------------------------------------------------
// Shared HTTP helpers
// ---------------------------------------------------------------------------

/// Build a GET request with common Bilibili headers.
fn bili_get(url: &str) -> ureq::Request {
    ureq::get(url)
        .set("User-Agent", UA)
        .set("Referer", REFERER)
        .timeout(std::time::Duration::from_secs(TIMEOUT))
}

/// Unwrap the Bilibili API envelope: `{"code": 0, "data": ...}`.
fn unwrap_data(json: &Value) -> Result<&Value, String> {
    match json.get("code").and_then(|c| c.as_i64()) {
        Some(0) => match json.get("data") {
            Some(d) => Ok(d),
            None => Err("API returned code=0 but no 'data' field".to_string()),
        },
        Some(code) => Err(format!("API returned code={}, message={:?}", code, json.get("message"))),
        None => Err("API response missing 'code' field".to_string()),
    }
}

// ---------------------------------------------------------------------------
// API endpoint URLs
// ---------------------------------------------------------------------------

/// Search (already existed): `keyword=<query>`, supports `page`.
fn search_url(query: &str, page: u32) -> String {
    format!(
        "https://api.bilibili.com/x/web-interface/search/all/v2?keyword={}&page={}",
        url_escape(query),
        page
    )
}

/// Video detail: `view?bvid=<bvid>`.
fn video_detail_url(bvid: &str) -> String {
    format!("https://api.bilibili.com/x/web-interface/view?bvid={}", bvid)
}

/// Video detail (richer): `view/detail?bvid=<bvid>`.
fn video_info_url(bvid: &str) -> String {
    format!("https://api.bilibili.com/x/web-interface/view/detail?bvid={}", bvid)
}

/// Hot ranking: `ranking/v2?rid=<rid>&type=all`.
fn ranking_url(rid: u32) -> String {
    format!(
        "https://api.bilibili.com/x/web-interface/ranking/v2?rid={}&type=all",
        rid
    )
}

/// User videos: `space/wbi/arc/search?mid=<mid>&ps=<ps>&pn=<pn>`.
fn user_videos_url(mid: u64, page: u32, ps: u32) -> String {
    format!(
        "https://api.bilibili.com/x/space/wbi/arc/search?mid={}&ps={}&pn={}",
        mid, ps, page
    )
}

/// User info: `space/wbi/acc/info?mid=<mid>`.
fn user_info_url(mid: u64) -> String {
    format!("https://api.bilibili.com/x/space/wbi/acc/info?mid={}", mid)
}

// ---------------------------------------------------------------------------
// URL encoding helper
// ---------------------------------------------------------------------------

fn url_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                escaped.push(byte as char);
            }
            b' ' => escaped.push('+'),
            other => {
                escaped.push('%');
                escaped.push(hex((other >> 4) & 0xf));
                escaped.push(hex(other & 0xf));
            }
        }
    }
    escaped
}

fn hex(d: u8) -> char {
    match d {
        0..=9 => (b'0' + d) as char,
        _ => (b'A' + (d - 10)) as char,
    }
}

// ---------------------------------------------------------------------------
// BilibiliChannel
// ---------------------------------------------------------------------------

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

    // -- native API methods ------------------------------------------------

    /// Search Bilibili with the public search API.
    /// Returns the raw JSON `data` field as `serde_json::Value`.
    pub fn search(query: &str, limit: usize) -> Result<Value, String> {
        // Fetch one or two pages to satisfy `limit`.
        let max_page = ((limit + 19) / 20).max(1).min(5); // API returns ≈20 per page
        let mut all_results: Vec<Value> = Vec::new();

        for page in 1..=max_page {
            let url = search_url(query, page as u32);
            let resp = bili_get(&url)
                .call()
                .map_err(|e| format!("search HTTP error: {}", e))?;
            let json: Value = resp
                .into_json()
                .map_err(|e| format!("search JSON parse error: {}", e))?;
            let data = unwrap_data(&json)?;

            // The search response nests results under `data.result[]` where
            // each entry has a `result_type` and `data` sub-object.
            // We extract the first layer of results for convenience.
            if let Some(result_arr) = data.get("result").and_then(|r| r.as_array()) {
                for item in result_arr {
                    if let Some(inner) = item.get("data") {
                        all_results.push(inner.clone());
                        if all_results.len() >= limit {
                            break;
                        }
                    }
                }
            }
            if all_results.len() >= limit {
                break;
            }
            if data
                .get("numResults")
                .and_then(|n| n.as_u64())
                .unwrap_or(0)
                < 20
            {
                break;
            }
        }

        all_results.truncate(limit);
        Ok(serde_json::to_value(all_results).unwrap_or(Value::Null))
    }

    /// Get video detail by BV号.
    pub fn get_video_detail(bvid: &str) -> Result<Value, String> {
        let url = video_detail_url(bvid);
        let resp = bili_get(&url)
            .call()
            .map_err(|e| format!("video_detail HTTP error: {}", e))?;
        let json: Value = resp
            .into_json()
            .map_err(|e| format!("video_detail JSON parse error: {}", e))?;
        unwrap_data(&json).cloned()
    }

    /// Get richer video detail (includes related videos, etc.) by BV号.
    pub fn get_video_info(bvid: &str) -> Result<Value, String> {
        let url = video_info_url(bvid);
        let resp = bili_get(&url)
            .call()
            .map_err(|e| format!("video_info HTTP error: {}", e))?;
        let json: Value = resp
            .into_json()
            .map_err(|e| format!("video_info JSON parse error: {}", e))?;
        unwrap_data(&json).cloned()
    }

    /// Get the hot ranking.
    ///
    /// `rid`: 0 = all categories, or a specific category id.
    /// `limit`: max number of results to return.
    pub fn get_ranking(rid: u32, limit: usize) -> Result<Value, String> {
        let url = ranking_url(rid);
        let resp = bili_get(&url)
            .call()
            .map_err(|e| format!("ranking HTTP error: {}", e))?;
        let json: Value = resp
            .into_json()
            .map_err(|e| format!("ranking JSON parse error: {}", e))?;
        let data = unwrap_data(&json)?;

        if let Some(list) = data.get("list").and_then(|l| l.as_array()) {
            let truncated: Vec<&Value> = list.iter().take(limit).collect();
            Ok(serde_json::to_value(truncated).unwrap_or(Value::Null))
        } else {
            // Some ranking endpoints return a flat array directly
            if let Some(arr) = data.as_array() {
                let truncated: Vec<&Value> = arr.iter().take(limit).collect();
                Ok(serde_json::to_value(truncated).unwrap_or(Value::Null))
            } else {
                Ok(data.clone())
            }
        }
    }

    /// Get videos uploaded by a user (UP主).
    ///
    /// `mid`: user id.
    /// `limit`: max number of results to return.
    pub fn get_user_videos(mid: u64, limit: usize) -> Result<Value, String> {
        let ps = (limit.min(30) as u32).max(1); // API supports ps up to 30
        let url = user_videos_url(mid, 1, ps);
        let resp = bili_get(&url)
            .call()
            .map_err(|e| format!("user_videos HTTP error: {}", e))?;
        let json: Value = resp
            .into_json()
            .map_err(|e| format!("user_videos JSON parse error: {}", e))?;
        let data = unwrap_data(&json)?;

        if let Some(vlist) = data
            .get("list")
            .and_then(|l| l.get("vlist"))
            .and_then(|v| v.as_array())
        {
            let truncated: Vec<&Value> = vlist.iter().take(limit).collect();
            Ok(serde_json::to_value(truncated).unwrap_or(Value::Null))
        } else {
            Ok(data.clone())
        }
    }

    /// Get user info by mid (UP主信息).
    pub fn get_user_info(mid: u64) -> Result<Value, String> {
        let url = user_info_url(mid);
        let resp = bili_get(&url)
            .call()
            .map_err(|e| format!("user_info HTTP error: {}", e))?;
        let json: Value = resp
            .into_json()
            .map_err(|e| format!("user_info JSON parse error: {}", e))?;
        unwrap_data(&json).cloned()
    }

    // -- backend probes ----------------------------------------------------

    /// Probe the native search API. Returns true if reachable and returning code=0.
    fn native_api_ok() -> bool {
        let url = search_url("test", 1);
        let resp = bili_get(&url).call();
        match resp {
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
}

// ---------------------------------------------------------------------------
// Channel trait impl
// ---------------------------------------------------------------------------

impl Channel for BilibiliChannel {
    fn name(&self) -> &str {
        "bilibili"
    }

    fn description(&self) -> &str {
        "B站视频、字幕和搜索"
    }

    fn backends(&self) -> &[&str] {
        &["B站 API (native)", "bili-cli", "OpenCLI"]
    }

    fn tier(&self) -> u8 {
        0
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

        let candidates = self.ordered_backends(config);
        let mut findings: Vec<(&str, CheckStatus, String)> = Vec::new();

        for backend in &candidates {
            let backend_str = backend.as_str();
            let result = match backend_str {
                "B站 API (native)" => {
                    if BilibiliChannel::native_api_ok() {
                        Some((
                            CheckStatus::Ok,
                            "B站 API (native) — 搜索/视频详情/热门排行/UP主信息，无需登录".to_string(),
                        ))
                    } else {
                        None
                    }
                }
                "bili-cli" => self.check_bili_cli(),
                "OpenCLI" => self.check_opencli(),
                _ => None,
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

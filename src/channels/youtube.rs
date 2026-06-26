//! YouTube — native InnerTube API (preferred) with yt-dlp fallback.
//!
//! Backends (ordered):
//! 1. `youtube-native` — zero external deps, uses YouTube's InnerTube API directly
//! 2. `yt-dlp`         — legacy fallback (requires Python + yt-dlp + JS runtime)
//!
//! InnerTube API reference:
//! - `/youtubei/v1/player`  — video metadata + streaming URLs + caption tracks
//! - `/youtubei/v1/search`  — search
//! - `/youtubei/v1/browse`  — channel page

use std::time::Duration;

use serde_json::{json, Value};
use url::Url;

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{command_exists, probe_command, ProbeStatus};
use crate::utils::paths::{get_ytdlp_config_path, render_ytdlp_fix_command};
use crate::utils::text::read_utf8_text;

// ── constants ──────────────────────────────────────────────────────────

const INNERTUBE_BASE: &str = "https://www.youtube.com/youtubei/v1";
const INNERTUBE_API_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";
const INNERTUBE_CLIENT_NAME: &str = "WEB";
const INNERTUBE_CLIENT_VERSION: &str = "2.20250623.00.00";

const UA: &str = "Mozilla/5.0 (compatible; agent-reach/1.5.0)";
const TIMEOUT_SECS: u64 = 15;
const PROBE_VIDEO_ID: &str = "dQw4w9WgXcQ"; // well-known permanent video

/// Check whether the yt-dlp user config explicitly enables a JS runtime.
fn has_js_runtime_config(config_path: &std::path::Path) -> bool {
    if !config_path.exists() {
        return false;
    }
    read_utf8_text(config_path)
        .map(|text| text.contains("--js-runtimes"))
        .unwrap_or(false)
}

// ── helpers ────────────────────────────────────────────────────────────

/// Build the `context` object sent with every InnerTube request.
fn inner_tube_context() -> Value {
    json!({
        "client": {
            "clientName": INNERTUBE_CLIENT_NAME,
            "clientVersion": INNERTUBE_CLIENT_VERSION,
        }
    })
}

/// POST JSON to an InnerTube endpoint, return the parsed response.
fn inner_tube_post(endpoint: &str, body: &Value) -> Result<Value, String> {
    let url = format!("{}/{}?key={}", INNERTUBE_BASE, endpoint, INNERTUBE_API_KEY);
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .set("User-Agent", UA)
        .set("X-YouTube-Client-Name", "1")
        .set("X-YouTube-Client-Version", INNERTUBE_CLIENT_VERSION)
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .send_json(body.clone())
        .map_err(|e| format!("InnerTube {} request failed: {}", endpoint, e))?;

    resp.into_json::<Value>()
        .map_err(|e| format!("InnerTube {} response parse error: {}", endpoint, e))
}

/// Simple GET returning a string body (for subtitle download).
fn simple_get(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .call()
        .map_err(|e| format!("HTTP GET failed: {}", e))?;
    resp.into_string()
        .map_err(|e| format!("HTTP response body error: {}", e))
}

/// Probe the native InnerTube API with a known-good video.
/// Returns true if the player endpoint responds with a valid `videoDetails`.
fn probe_native_api() -> bool {
    let body = json!({
        "videoId": PROBE_VIDEO_ID,
        "context": inner_tube_context(),
    });
    match inner_tube_post("player", &body) {
        Ok(val) => val.get("videoDetails").is_some(),
        Err(_) => false,
    }
}

// ── YouTubeChannel ─────────────────────────────────────────────────────

pub struct YouTubeChannel {
    pub active_backend: Option<String>,
}

impl YouTubeChannel {
    pub fn new() -> Self {
        YouTubeChannel {
            active_backend: None,
        }
    }

    // ── InnerTube API methods ───────────────────────────────────────

    /// Get video metadata (title, author, length, description, etc.)
    /// from the InnerTube player endpoint.
    ///
    /// Returns the full parsed JSON response on success.
    pub fn get_video_info(&self, video_id: &str) -> Result<Value, String> {
        let body = json!({
            "videoId": video_id,
            "context": inner_tube_context(),
        });
        inner_tube_post("player", &body)
    }

    /// Get subtitles (captions) for a video in the requested language.
    ///
    /// `lang` is a language code like `"en"`, `"zh-Hans"`, etc.
    /// Falls back to the first available track if the requested language
    /// is not found.
    ///
    /// Returns the subtitle text (json3 format parsed to plain text).
    pub fn get_subtitles(&self, video_id: &str, lang: &str) -> Result<String, String> {
        // 1. Get player response to find caption tracks
        let player = self.get_video_info(video_id)?;

        let tracks = player
            .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
            .and_then(|v| v.as_array())
            .ok_or_else(|| "No caption tracks available for this video".to_string())?;

        if tracks.is_empty() {
            return Err("No caption tracks found".to_string());
        }

        // 2. Find the requested language track, or fall back to first
        let track = tracks
            .iter()
            .find(|t| {
                t.get("languageCode")
                    .and_then(|v| v.as_str())
                    .map(|code| code.eq_ignore_ascii_case(lang))
                    .unwrap_or(false)
            })
            .unwrap_or(&tracks[0]);

        let base_url = track
            .get("baseUrl")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Caption track missing baseUrl".to_string())?;

        // 3. Download captions (json3 is the most parseable format)
        let caption_url = format!("{}&fmt=json3", base_url);
        let raw = simple_get(&caption_url)?;

        // 4. Parse json3 events into plain text
        let events: Value = serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse caption JSON: {}", e))?;

        let text = events
            .get("events")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|ev| {
                        ev.get("segs")
                            .and_then(|s| s.as_array())
                            .map(|segs| {
                                segs.iter()
                                    .filter_map(|seg| seg.get("utf8").and_then(|u| u.as_str()))
                                    .collect::<Vec<_>>()
                                    .join("")
                            })
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        Ok(text)
    }

    /// Search YouTube for videos matching the query.
    ///
    /// `limit` caps the number of results returned (inner response usually
    /// contains ~20 items; this method truncates to `limit`).
    ///
    /// Returns a Vec of video renderer objects from the response.
    pub fn search_videos(&self, query: &str, limit: usize) -> Result<Vec<Value>, String> {
        let body = json!({
            "query": query,
            "params": "EgIQAQ%3D%3D",  // filter: videos only
            "context": inner_tube_context(),
        });
        let resp = inner_tube_post("search", &body)?;

        // Walk into contents.twoColumnSearchResultsRenderer.primaryContents
        //                      .sectionListRenderer.contents[0]
        //                      .itemSectionRenderer.contents
        let items: Vec<Value> = resp
            .pointer("/contents/twoColumnSearchResultsRenderer/primaryContents/sectionListRenderer/contents/0/itemSectionRenderer/contents")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .take(limit)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();

        Ok(items)
    }
}

// ── Channel trait impl ─────────────────────────────────────────────────

impl Channel for YouTubeChannel {
    fn name(&self) -> &str {
        "youtube"
    }

    fn description(&self) -> &str {
        "YouTube 视频和字幕"
    }

    fn backends(&self) -> &[&str] {
        &["youtube-native", "yt-dlp"]
    }

    fn tier(&self) -> u8 {
        0
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_lowercase();
                host.contains("youtube.com") || host.contains("youtu.be")
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
        // ── 1. Try native InnerTube API ─────────────────────────────
        if probe_native_api() {
            self.active_backend = Some("youtube-native".to_string());

            let mut msg = String::from("通过 YouTube InnerTube API 提取视频信息、字幕和搜索（零外部依赖）");

            // Surface transcription readiness if whisper config exists
            if let Some(cfg) = config {
                let mut providers: Vec<&str> = Vec::new();
                if cfg.is_configured("groq_whisper") {
                    providers.push("groq");
                }
                if cfg.is_configured("openai_whisper") {
                    providers.push("openai");
                }

                if !providers.is_empty() {
                    if !command_exists("ffmpeg") {
                        msg.push_str("（音频转写需安装 ffmpeg）");
                    } else {
                        use std::fmt::Write;
                        let _ = write!(msg, "，可转写音频（{}）", providers.join("→"));
                    }
                }
            }

            return CheckResult {
                status: CheckStatus::Ok,
                message: msg,
                active_backend: self.active_backend.clone(),
            };
        }

        // ── 2. Fall through to yt-dlp probe ────────────────────────
        let probe = probe_command("yt-dlp", &["--version"], 10, 0, Some("yt-dlp"));

        if probe.status == ProbeStatus::Missing {
            self.active_backend = None;
            return CheckResult {
                status: CheckStatus::Off,
                message: "YouTube InnerTube API 不可用，且 yt-dlp 未安装。安装：pip install yt-dlp".to_string(),
                active_backend: None,
            };
        }

        if probe.status == ProbeStatus::Broken {
            self.active_backend = None;
            return CheckResult {
                status: CheckStatus::Error,
                message: format!(
                    "YouTube InnerTube API 不可用，yt-dlp 已安装但无法执行\n{}",
                    probe.hint
                ),
                active_backend: None,
            };
        }

        if !probe.ok() {
            self.active_backend = None;
            let detail = if !probe.hint.is_empty() {
                probe.hint
            } else if !probe.output.is_empty() {
                probe.output
            } else {
                probe.status.as_str().to_string()
            };
            return CheckResult {
                status: CheckStatus::Error,
                message: format!(
                    "YouTube InnerTube API 不可用，yt-dlp 无法正常运行：{}",
                    detail
                ),
                active_backend: None,
            };
        }

        // yt-dlp is alive — activate it as fallback
        self.active_backend = Some("yt-dlp".to_string());

        // Check JS runtime
        let has_js = command_exists("deno") || command_exists("node");
        if !has_js {
            return CheckResult {
                status: CheckStatus::Warn,
                message: "yt-dlp 已安装但缺少 JS runtime（YouTube 必须）。\n  安装 Node.js 或 deno，然后运行：agent-reach install".to_string(),
                active_backend: self.active_backend.clone(),
            };
        }

        // Check yt-dlp config for --js-runtimes
        let has_deno = command_exists("deno");
        if !has_deno {
            let ytdlp_config = get_ytdlp_config_path();
            if !has_js_runtime_config(&ytdlp_config) {
                return CheckResult {
                    status: CheckStatus::Warn,
                    message: format!(
                        "yt-dlp 已安装但未配置 JS runtime。运行：\n  {}",
                        render_ytdlp_fix_command()
                    ),
                    active_backend: self.active_backend.clone(),
                };
            }
        }

        let mut msg = String::from("通过 yt-dlp 提取视频信息和字幕（需 JS runtime）");

        if let Some(cfg) = config {
            let mut providers: Vec<&str> = Vec::new();
            if cfg.is_configured("groq_whisper") {
                providers.push("groq");
            }
            if cfg.is_configured("openai_whisper") {
                providers.push("openai");
            }

            if !providers.is_empty() {
                if !command_exists("ffmpeg") {
                    msg.push_str("（音频转写需安装 ffmpeg）");
                } else {
                    use std::fmt::Write;
                    let _ = write!(msg, "，可转写音频（{}）", providers.join("→"));
                }
            }
        }

        CheckResult {
            status: CheckStatus::Ok,
            message: msg,
            active_backend: self.active_backend.clone(),
        }
    }
}

//! YouTube — check if yt-dlp is available with JS runtime.

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{command_exists, probe_command, ProbeStatus};
use crate::utils::paths::{get_ytdlp_config_path, render_ytdlp_fix_command};
use crate::utils::text::read_utf8_text;
use url::Url;

/// Check whether the yt-dlp user config explicitly enables a JS runtime.
fn has_js_runtime_config(config_path: &std::path::Path) -> bool {
    if !config_path.exists() {
        return false;
    }
    read_utf8_text(config_path)
        .map(|text| text.contains("--js-runtimes"))
        .unwrap_or(false)
}

/// YouTube channel — extract video info, subtitles, and optionally transcribe audio.
pub struct YouTubeChannel {
    pub active_backend: Option<String>,
}

impl YouTubeChannel {
    pub fn new() -> Self {
        YouTubeChannel {
            active_backend: None,
        }
    }
}

impl Channel for YouTubeChannel {
    fn name(&self) -> &str {
        "youtube"
    }

    fn description(&self) -> &str {
        "YouTube 视频和字幕"
    }

    fn backends(&self) -> &[&str] {
        &["yt-dlp"]
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
        // Actually run yt-dlp --version to distinguish missing / broken venv / runtime failure.
        let probe = probe_command("yt-dlp", &["--version"], 10, 0, Some("yt-dlp"));

        if probe.status == ProbeStatus::Missing {
            self.active_backend = None;
            return CheckResult {
                status: CheckStatus::Off,
                message: "yt-dlp 未安装。安装：pip install yt-dlp".to_string(),
                active_backend: None,
            };
        }

        if probe.status == ProbeStatus::Broken {
            self.active_backend = None;
            return CheckResult {
                status: CheckStatus::Error,
                message: format!("yt-dlp 已安装但无法执行\n{}", probe.hint),
                active_backend: None,
            };
        }

        if !probe.ok() {
            // timeout / error: installed but won't run
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
                message: format!("yt-dlp 无法正常运行：{}", detail),
                active_backend: None,
            };
        }

        // yt-dlp itself is alive; subsequent JS runtime / transcription checks
        // only affect ok vs warn, not backend ownership.
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
        // Deno works out of the box; Node.js requires explicit config
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

        // Surface transcription readiness so `doctor` reports it.
        let mut msg = String::from("可提取视频信息和字幕");

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

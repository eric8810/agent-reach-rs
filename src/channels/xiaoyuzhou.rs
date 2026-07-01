//! Xiaoyuzhou Podcast (小宇宙播客) — transcribe podcasts via Groq/OpenAI Whisper API.
//!
//! Multi-backend architecture:
//!   1. Whisper API (native) — zero external deps, only needs Groq/OpenAI API key
//!   2. groq-whisper — full local pipeline (ffmpeg + transcribe script + API key)
//!   3. ffmpeg — audio tool present (secondary concern)
//!
//! The API call itself works without ffmpeg; ffmpeg is only needed for
//! audio download and processing.

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use url::Url;

/// Xiaoyuzhou podcast channel — multi-backend with tiered fallback.
pub struct XiaoyuzhouChannel {
    pub active_backend: Option<String>,
}

impl XiaoyuzhouChannel {
    pub fn new() -> Self {
        XiaoyuzhouChannel {
            active_backend: None,
        }
    }

    // ── Whisper API (native) backend ──────────────────────────────────

    /// Probe the native Whisper API backend.
    ///
    /// Only needs a Groq (or OpenAI) API key — no ffmpeg, no external script.
    /// Returns None when no key is configured.
    fn check_whisper_native(config: Option<&Config>) -> Option<(CheckStatus, String)> {
        let has_groq = std::env::var("GROQ_API_KEY")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
            || config
                .and_then(|cfg| cfg.get("groq_api_key"))
                .map(|v| !v.is_empty())
                .unwrap_or(false);

        let has_openai = std::env::var("OPENAI_API_KEY")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
            || config
                .and_then(|cfg| cfg.get("openai_api_key"))
                .map(|v| !v.is_empty())
                .unwrap_or(false);

        if has_groq || has_openai {
            let provider = if has_groq { "Groq" } else { "OpenAI" };
            Some((
                CheckStatus::Ok,
                format!(
                    "Whisper API (native) 可用 — 通过 {} API 直接转录，无需 ffmpeg 或本地脚本。",
                    provider
                ),
            ))
        } else {
            None
        }
    }

    // ── groq-whisper backend (full local pipeline) ─────────────────────

    /// Probe the full groq-whisper pipeline (ffmpeg + transcribe script + API key).
    fn check_groq_whisper(config: Option<&Config>) -> Option<(CheckStatus, String)> {
        // Check ffmpeg (auto-download if needed)
        if crate::ffmpeg_dl::find_ffmpeg().is_none() && crate::ffmpeg_dl::ensure_ffmpeg(false).is_err() {
            return Some((
                CheckStatus::Off,
                "需要 ffmpeg（音频转码和切片）。运行 agent-reach install 自动下载。".to_string(),
            ));
        }

        // Check script exists
        let script = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".agent-reach")
            .join("tools")
            .join("xiaoyuzhou")
            .join("transcribe.sh");
        if !script.is_file() {
            return Some((
                CheckStatus::Off,
                "转录脚本未安装。运行：\n  agent-reach install --env=auto\n  或手动复制 transcribe.sh 到 ~/.agent-reach/tools/xiaoyuzhou/".to_string(),
            ));
        }

        // Check API key
        let has_key = std::env::var("GROQ_API_KEY")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
            || config
                .and_then(|cfg| cfg.get("groq_api_key"))
                .map(|v| !v.is_empty())
                .unwrap_or(false);

        if !has_key {
            return Some((
                CheckStatus::Warn,
                "需要配置 Groq API Key（免费）。步骤：\n  1. 注册 https://console.groq.com\n  2. 运行: agent-reach configure groq-key gsk_xxxxx".to_string(),
            ));
        }

        Some((
            CheckStatus::Ok,
            "完整可用（播客下载 + Whisper 转录）".to_string(),
        ))
    }

    // ── ffmpeg-only backend ────────────────────────────────────────────

    /// Probe ffmpeg presence only (secondary concern for audio processing).
    fn check_ffmpeg() -> Option<(CheckStatus, String)> {
        if crate::ffmpeg_dl::find_ffmpeg().is_some() || crate::ffmpeg_dl::ensure_ffmpeg(false).is_ok() {
            Some((CheckStatus::Ok, "ffmpeg 可用（音频转码和切片）".to_string()))
        } else {
            Some((CheckStatus::Off, "ffmpeg 未安装。运行 agent-reach install 自动下载。".to_string()))
        }
    }
}

impl Channel for XiaoyuzhouChannel {
    fn name(&self) -> &str {
        "xiaoyuzhou"
    }

    fn description(&self) -> &str {
        "小宇宙播客转文字"
    }

    fn backends(&self) -> &[&str] {
        &["Whisper API (native)", "groq-whisper", "ffmpeg"]
    }

    fn tier(&self) -> u8 {
        0 // Whisper API (native) is zero-config when API key is set
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_lowercase();
                host.contains("xiaoyuzhoufm.com")
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
        let mut findings: Vec<(String, CheckStatus, String)> = Vec::new();

        for backend in self.ordered_backends(config) {
            let result = match backend.as_str() {
                "Whisper API (native)" => Self::check_whisper_native(config),
                "groq-whisper" => Self::check_groq_whisper(config),
                "ffmpeg" => Self::check_ffmpeg(),
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
                "小宇宙播客未配置。配置方式：\n",
                "  1. 设置 groq_api_key 或 openai_api_key → 使用原生 Whisper API（推荐，零外部依赖）\n",
                "  2. 安装 ffmpeg + 转录脚本 → 完整本地管道\n",
                "     Ubuntu/Debian: apt install -y ffmpeg\n",
                "     macOS: brew install ffmpeg\n",
                "     运行: agent-reach install --env=auto"
            )
            .to_string(),
            active_backend: None,
        }
    }
}

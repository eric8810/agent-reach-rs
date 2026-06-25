//! Xiaoyuzhou Podcast (小宇宙播客) — transcribe podcasts via Groq Whisper API.

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{probe_command, ProbeStatus};
use url::Url;

/// Xiaoyuzhou podcast channel — requires ffmpeg + transcribe script + Groq API key.
pub struct XiaoyuzhouChannel {
    pub active_backend: Option<String>,
}

impl XiaoyuzhouChannel {
    pub fn new() -> Self {
        XiaoyuzhouChannel {
            active_backend: None,
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
        &["groq-whisper", "ffmpeg"]
    }

    fn tier(&self) -> u8 {
        1
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

        // Check ffmpeg — really execute it: a stale pip-installed ffmpeg shim
        // passes which() but cannot run.
        let probe = probe_command("ffmpeg", &["-version"], 10, 0, Some("ffmpeg"));
        if probe.status == ProbeStatus::Missing {
            return CheckResult {
                status: CheckStatus::Off,
                message: "需要 ffmpeg（音频转码和切片）。安装：\n  Ubuntu/Debian: apt install -y ffmpeg\n  macOS: brew install ffmpeg".to_string(),
                active_backend: None,
            };
        }
        if !probe.ok() {
            return CheckResult {
                status: CheckStatus::Error,
                message: "ffmpeg 无法执行，重装：brew install ffmpeg（macOS）/ apt install ffmpeg（Linux）".to_string(),
                active_backend: None,
            };
        }

        // Check script exists
        let script = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".agent-reach")
            .join("tools")
            .join("xiaoyuzhou")
            .join("transcribe.sh");
        if !script.is_file() {
            return CheckResult {
                status: CheckStatus::Off,
                message: "转录脚本未安装。运行：\n  agent-reach install --env=auto\n  或手动复制 transcribe.sh 到 ~/.agent-reach/tools/xiaoyuzhou/".to_string(),
                active_backend: None,
            };
        }

        // Check GROQ_API_KEY — prefer env var, fall back to Agent Reach config
        let has_key = std::env::var("GROQ_API_KEY")
            .map(|v| !v.is_empty())
            .unwrap_or(false)
            || config
                .and_then(|cfg| cfg.get("groq_api_key"))
                .map(|v| !v.is_empty())
                .unwrap_or(false);

        if !has_key {
            return CheckResult {
                status: CheckStatus::Warn,
                message: "需要配置 Groq API Key（免费）。步骤：\n  1. 注册 https://console.groq.com\n  2. 运行: agent-reach configure groq-key gsk_xxxxx".to_string(),
                active_backend: None,
            };
        }

        self.active_backend = Some("groq-whisper".to_string());
        CheckResult {
            status: CheckStatus::Ok,
            message: "完整可用（播客下载 + Whisper 转录）".to_string(),
            active_backend: self.active_backend.clone(),
        }
    }
}

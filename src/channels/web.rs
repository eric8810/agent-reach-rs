//! Web — any URL via Jina Reader. Always available.

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;

const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36";

/// Web channel — fallback channel that handles any URL via Jina Reader.
pub struct WebChannel {
    active_backend: Option<String>,
}

impl WebChannel {
    pub fn new() -> Self {
        WebChannel { active_backend: None }
    }

    /// Read a web page via Jina Reader, returning Markdown text.
    pub fn read(&self, url: &str) -> anyhow::Result<String> {
        let url = if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("https://{}", url)
        } else {
            url.to_string()
        };
        let jina_url = format!("https://r.jina.ai/{}", url);
        let resp = ureq::get(&jina_url)
            .set("User-Agent", UA)
            .set("Accept", "text/plain")
            .timeout(std::time::Duration::from_secs(30))
            .call()?;
        Ok(resp.into_string()?)
    }
}

impl Channel for WebChannel {
    fn name(&self) -> &str {
        "web"
    }

    fn description(&self) -> &str {
        "任意网页"
    }

    fn backends(&self) -> &[&str] {
        &["Jina Reader"]
    }

    fn tier(&self) -> u8 {
        0
    }

    fn can_handle(&self, _url: &str) -> bool {
        true // Fallback — handles any URL
    }

    fn active_backend(&self) -> Option<String> {
        self.active_backend.clone()
    }

    fn set_active_backend(&mut self, backend: Option<String>) {
        self.active_backend = backend;
    }

    fn check(&mut self, _config: Option<&Config>) -> CheckResult {
        // Always available fallback channel: no local commands, no network probe
        // (doctor already touches the network via other channels), keep zero overhead.
        self.active_backend = Some(self.backends()[0].to_string());
        CheckResult {
            status: CheckStatus::Ok,
            message: "通过 Jina Reader 读取任意网页（curl https://r.jina.ai/URL）".to_string(),
            active_backend: self.active_backend.clone(),
        }
    }
}

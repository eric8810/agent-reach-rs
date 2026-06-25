//! RSS/Atom feed channel — reads RSS and Atom feeds.
//!
//! Uses the `rss` and `atom_syndication` crates, which are compile-time
//! dependencies always available when the binary compiles.

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;

/// Channel for RSS/Atom feed sources.
pub struct RSSChannel {
    active_backend: Option<String>,
}

impl RSSChannel {
    pub fn new() -> Self {
        RSSChannel { active_backend: None }
    }
}

impl Channel for RSSChannel {
    fn name(&self) -> &str {
        "rss"
    }

    fn description(&self) -> &str {
        "RSS/Atom 订阅源"
    }

    fn backends(&self) -> &[&str] {
        &["rss", "atom_syndication"]
    }

    fn tier(&self) -> u8 {
        0
    }

    fn can_handle(&self, url: &str) -> bool {
        let lower = url.to_lowercase();
        lower.contains("/feed")
            || lower.contains("/rss")
            || lower.contains(".xml")
            || lower.contains("atom")
    }

    fn active_backend(&self) -> Option<String> {
        self.active_backend.clone()
    }

    fn set_active_backend(&mut self, backend: Option<String>) {
        self.active_backend = backend;
    }

    fn check(&mut self, _config: Option<&Config>) -> CheckResult {
        // The `rss` and `atom_syndication` crates are compile-time
        // dependencies — if the binary compiled, they are always available.
        self.active_backend = Some(self.backends()[0].to_string());
        CheckResult {
            status: CheckStatus::Ok,
            message: "可读取 RSS/Atom 源".to_string(),
            active_backend: self.active_backend.clone(),
        }
    }
}

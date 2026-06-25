//! Channel base trait — platform availability checking.
//!
//! Each channel represents a platform (YouTube, Twitter, GitHub, etc.)
//! and provides:
//! - can_handle(url) → does this URL belong to this platform?
//! - check(config) → is the upstream tool installed and configured?
//!
//! After installation, agents call upstream tools directly.
//!
//! Backend routing semantics:
//! - `backends` is an ORDERED candidate list: backends[0] is the preferred
//!   backend, the rest are fallbacks.
//! - check() must set `self.active_backend` to the backend that is actually
//!   serving the channel right now (None when nothing usable is found).
//! - Users can force a backend with config key `<channel>_backend`
//!   (or env var `<CHANNEL>_BACKEND`); ordered_backends() applies it.

use crate::config::Config;

/// Result of a channel health check.
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub status: CheckStatus,
    pub message: String,
    pub active_backend: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    Ok,
    Warn,
    Off,
    Error,
}

impl CheckStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CheckStatus::Ok => "ok",
            CheckStatus::Warn => "warn",
            CheckStatus::Off => "off",
            CheckStatus::Error => "error",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "ok" => CheckStatus::Ok,
            "warn" => CheckStatus::Warn,
            "off" => CheckStatus::Off,
            _ => CheckStatus::Error,
        }
    }
}

/// Base trait for all channels.
pub trait Channel: Send + Sync {
    /// Short name, e.g. "youtube".
    fn name(&self) -> &str;

    /// Human-readable description, e.g. "YouTube 视频和字幕".
    fn description(&self) -> &str;

    /// Ordered candidate backends; backends[0] = preferred.
    fn backends(&self) -> &[&str];

    /// Tier: 0 = zero-config, 1 = needs free key/login, 2 = needs setup.
    fn tier(&self) -> u8;

    /// Check if this channel can handle this URL.
    fn can_handle(&self, url: &str) -> bool;

    /// Current active backend (set by check()).
    fn active_backend(&self) -> Option<String>;

    /// Set the active backend.
    fn set_active_backend(&mut self, backend: Option<String>);

    /// Get ordered backends, honoring user override from config.
    fn ordered_backends(&self, config: Option<&Config>) -> Vec<String> {
        let mut candidates: Vec<String> = self.backends().iter().map(|s| s.to_string()).collect();
        if let Some(config) = config {
            let key = format!("{}_backend", self.name());
            if let Some(override_val) = config.get(&key) {
                for (i, b) in candidates.iter().enumerate() {
                    if b == &override_val || b.starts_with(&override_val) {
                        let removed = candidates.remove(i);
                        candidates.insert(0, removed);
                        break;
                    }
                }
            }
        }
        candidates
    }

    /// Check if this channel's upstream tool is available.
    /// Returns (status, message) where status is 'ok'/'warn'/'off'/'error'.
    /// Must set self.active_backend.
    fn check(&mut self, config: Option<&Config>) -> CheckResult;
}

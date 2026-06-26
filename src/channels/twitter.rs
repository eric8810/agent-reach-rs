//! Twitter/X — multi-backend: native API / twitter-cli / OpenCLI.
//!
//! Backend order encodes the recommendation:
//! 1. Twitter API (native) — zero external deps, GraphQL API direct
//! 2. twitter-cli — dedicated Twitter CLI (pip package), full-featured
//! 3. OpenCLI — cross-platform via Chrome browser session

use serde_json::{json, Value};
use std::time::Duration;
use url::Url;

use crate::backends::{opencli_status, OpenCLIStatus};
use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{probe_command, ProbeStatus};

// ── GraphQL query IDs (community-documented, may change over time) ──────

/// SearchTimeline query ID.
const SEARCH_QUERY_ID: &str = "gkjsKepM6gl_HmFWoWKfgg";
/// TweetDetail query ID.
const TWEET_DETAIL_QUERY_ID: &str = "0hWvDhmW8YQ-S_ib3azIrw";
/// UserTweets query ID.
const USER_TWEETS_QUERY_ID: &str = "E3opETHemVhFLDO6N2JHxw";

// ── API constants ───────────────────────────────────────────────────────

/// Anonymous bearer token used by the official Twitter/X web client.
const ANON_BEARER: &str = "AAAAAAAAAAAAAAAAAAAAANRILgAAAAAAnNwIzUejRCOuH5E6I8xnZz4puTs%3D1Zv7ttfk8LF81IUq16cHjhLTvJu4FA33AGWWjCpTnA";

/// Twitter/X GraphQL API base URL.
const GRAPHQL_BASE: &str = "https://x.com/i/api/graphql";

/// Guest activation endpoint.
const GUEST_ACTIVATE_URL: &str = "https://api.x.com/1.1/guest/activate.json";

/// Lightweight probe query (1 result, no heavy timeline construction).
const PROBE_QUERY: &str = "hello";

/// Features blob sent alongside every GraphQL request.
const FEATURES_JSON: &str = r#"{"longform_notetweets_enabled":true,"responsive_web_enhance_cards_enabled":false,"tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled":true,"interactive_text_enabled":true,"responsive_web_text_conversations_enabled":false,"vibe_tweet_context_enabled":false,"subscriptions_verification_info_verified_since_enabled":true,"verified_phone_label_enabled":true,"subscriptions_verification_info_is_identity_verified_enabled":true,"responsive_web_graphql_timeline_navigation_enabled":true}"#;

// ── struct ──────────────────────────────────────────────────────────────

/// Twitter channel — multi-backend with native API, twitter-cli, and OpenCLI.
pub struct TwitterChannel {
    pub active_backend: Option<String>,
}

impl TwitterChannel {
    pub fn new() -> Self {
        TwitterChannel {
            active_backend: None,
        }
    }

    // ── native API: helpers ────────────────────────────────────────────

    /// Acquire a fresh guest token from Twitter's activate endpoint.
    fn fetch_guest_token() -> Result<String, String> {
        let resp = ureq::post(GUEST_ACTIVATE_URL)
            .set("Authorization", &format!("Bearer {}", ANON_BEARER))
            .set("User-Agent", "agent-reach/1.5")
            .timeout(Duration::from_secs(15))
            .call()
            .map_err(|e| format!("guest activate transport error: {}", e))?;

        let body = resp
            .into_string()
            .map_err(|e| format!("guest activate read error: {}", e))?;

        let parsed: Value = serde_json::from_str(&body)
            .map_err(|e| format!("guest activate JSON parse error: {}", e))?;

        parsed
            .get("guest_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                format!(
                    "guest activate response missing guest_token: {}",
                    &body.chars().take(200).collect::<String>()
                )
            })
    }

    /// Build a POST request to a GraphQL endpoint with standard headers.
    fn graphql_post(
        query_id: &str,
        endpoint: &str,
        body: Value,
        auth_token: Option<&str>,
        ct0: Option<&str>,
        guest_token: &str,
    ) -> Result<Value, String> {
        let url = format!("{}/{}/{}", GRAPHQL_BASE, query_id, endpoint);

        let mut req = ureq::post(&url)
            .set("Authorization", &format!("Bearer {}", ANON_BEARER))
            .set("Content-Type", "application/json")
            .set("User-Agent", "agent-reach/1.5")
            .set("x-twitter-active-user", "yes")
            .set("x-twitter-client-language", "en")
            .set("x-guest-token", guest_token)
            .timeout(Duration::from_secs(30));

        // Build cookie header if auth is configured
        let mut cookie_parts: Vec<String> = Vec::new();
        if let Some(tok) = auth_token {
            cookie_parts.push(format!("auth_token={}", tok));
        }
        if let Some(ct) = ct0 {
            cookie_parts.push(format!("ct0={}", ct));
        }
        if !cookie_parts.is_empty() {
            req = req.set("Cookie", &cookie_parts.join("; "));
        }

        match req.send_json(body) {
            Ok(resp) => {
                let body_str = resp
                    .into_string()
                    .map_err(|e| format!("GraphQL read error: {}", e))?;
                serde_json::from_str(&body_str)
                    .map_err(|e| format!("GraphQL JSON parse error: {}", e))
            }
            Err(ureq::Error::Status(code, resp)) => {
                let body_str = resp
                    .into_string()
                    .unwrap_or_else(|_| "<unreadable>".to_string());
                Err(format!(
                    "GraphQL HTTP {}: {}",
                    code,
                    body_str.chars().take(500).collect::<String>()
                ))
            }
            Err(ureq::Error::Transport(e)) => {
                Err(format!("GraphQL transport error: {}", e))
            }
        }
    }

    // ── native API: public data-fetching methods ───────────────────────

    /// Search tweets by query string.
    /// Returns the raw JSON response from the GraphQL API.
    pub fn search_tweets(
        query: &str,
        limit: usize,
        auth_token: Option<&str>,
        ct0: Option<&str>,
    ) -> Result<Value, String> {
        let guest_token = Self::fetch_guest_token()?;

        let variables = json!({
            "rawQuery": query,
            "count": limit.min(20),
            "product": "Top",
            "querySource": "typed_query"
        });

        let features: Value = serde_json::from_str(FEATURES_JSON)
            .map_err(|e| format!("features JSON parse: {}", e))?;

        let body = json!({
            "variables": variables,
            "features": features
        });

        Self::graphql_post(
            SEARCH_QUERY_ID,
            "SearchTimeline",
            body,
            auth_token,
            ct0,
            &guest_token,
        )
    }

    /// Get a single tweet by ID.
    pub fn get_tweet(
        tweet_id: &str,
        auth_token: Option<&str>,
        ct0: Option<&str>,
    ) -> Result<Value, String> {
        let guest_token = Self::fetch_guest_token()?;

        let variables = json!({
            "focalTweetId": tweet_id,
            "with_rux_injections": false,
            "includePromotedContent": true,
            "withCommunity": true,
            "withQuickPromoteEligibilityTweetFields": true,
            "withBirdwatchNotes": false,
            "withVoice": true,
            "withV2Timeline": true
        });

        let features: Value = serde_json::from_str(FEATURES_JSON)
            .map_err(|e| format!("features JSON parse: {}", e))?;

        let field_toggles = json!({
            "withArticleRichContentState": true,
            "withArticlePlainText": false,
            "withGrokAnalyze": false
        });

        let body = json!({
            "variables": variables,
            "features": features,
            "fieldToggles": field_toggles
        });

        Self::graphql_post(
            TWEET_DETAIL_QUERY_ID,
            "TweetDetail",
            body,
            auth_token,
            ct0,
            &guest_token,
        )
    }

    /// Get tweets from a user's timeline by username.
    /// First resolves the username to a user ID via a search, then fetches
    /// their timeline. Returns the raw timeline JSON (UserTweets response).
    pub fn get_user_tweets(
        username: &str,
        limit: usize,
        auth_token: Option<&str>,
        ct0: Option<&str>,
    ) -> Result<Value, String> {
        // Resolve user ID: search for "from:{username}" and extract the
        // first result's user id from core.user_results.result.rest_id.
        let search_result = Self::search_tweets(
            &format!("from:{}", username),
            1,
            auth_token,
            ct0,
        )?;

        let user_id = search_result
            .pointer("/data/search_by_raw_query/search_timeline/timeline/instructions")
            .and_then(|instrs| instrs.as_array())
            .and_then(|arr| {
                // Find the first TimelineAddEntries instruction
                arr.iter().find_map(|entry| {
                    let entries = entry
                        .get("entries")
                        .and_then(|e| e.as_array())?;
                    entries.iter().find_map(|e| {
                        e.pointer("/content/itemContent/tweet_results/result/core/user_results/result/rest_id")
                            .and_then(|v| v.as_str())
                    })
                })
            })
            .map(|s| s.to_string())
            .ok_or_else(|| {
                format!(
                    "Could not resolve user ID for @{}. Try providing a user ID directly.",
                    username
                )
            })?;

        let guest_token = Self::fetch_guest_token()?;

        let variables = json!({
            "userId": user_id,
            "count": limit.min(20),
            "includePromotedContent": true,
            "withQuickPromoteEligibilityTweetFields": true,
            "withVoice": true,
            "withV2Timeline": true
        });

        let features: Value = serde_json::from_str(FEATURES_JSON)
            .map_err(|e| format!("features JSON parse: {}", e))?;

        let field_toggles = json!({
            "withArticleRichContentState": true,
            "withArticlePlainText": false,
            "withGrokAnalyze": false
        });

        let body = json!({
            "variables": variables,
            "features": features,
            "fieldToggles": field_toggles
        });

        Self::graphql_post(
            USER_TWEETS_QUERY_ID,
            "UserTweets",
            body,
            auth_token,
            ct0,
            &guest_token,
        )
    }

    // ── native API: health check ───────────────────────────────────────

    /// Check the native Twitter GraphQL API backend.
    fn check_native_api(
        &self,
        config: Option<&Config>,
    ) -> Option<(String, String)> {
        let auth_token = config.and_then(|c| c.get("twitter_auth_token"));
        let ct0 = config.and_then(|c| c.get("twitter_ct0"));

        // Try a lightweight probe: search for a single tweet
        match Self::search_tweets(
            PROBE_QUERY,
            1,
            auth_token.as_deref(),
            ct0.as_deref(),
        ) {
            Ok(_resp) => {
                let has_auth = auth_token.as_ref().map_or(false, |t| !t.is_empty());
                if has_auth {
                    Some((
                        "ok".to_string(),
                        "Twitter API (native) 可用（GraphQL，零外部依赖，已登录）".to_string(),
                    ))
                } else {
                    Some((
                        "warn".to_string(),
                        concat!(
                            "Twitter API (native) 可用，但未配置登录凭据（仅匿名访问）。\n",
                            "  设置方式：\n",
                            "    在 ~/.agent-reach/config.yaml 中添加：\n",
                            "      twitter_auth_token: \"xxx\"\n",
                            "      twitter_ct0: \"yyy\"\n",
                            "  或设置环境变量 TWITTER_AUTH_TOKEN / TWITTER_CT0\n",
                            "  获取方式：浏览器登录 x.com → DevTools → Application → Cookies"
                        )
                        .to_string(),
                    ))
                }
            }
            Err(e) => {
                // If the error is clearly auth-related and we have no token,
                // don't report as error — just warn about missing config
                let has_auth = auth_token.as_ref().map_or(false, |t| !t.is_empty());
                if !has_auth {
                    Some((
                        "warn".to_string(),
                        format!(
                            "Twitter API (native) 不可用：{}\n需要配置 twitter_auth_token 和 twitter_ct0。",
                            e
                        ),
                    ))
                } else {
                    Some((
                        "error".to_string(),
                        format!("Twitter API (native) 请求失败：{}", e),
                    ))
                }
            }
        }
    }

    // ── twitter-cli probe ──────────────────────────────────────────────

    /// Probe twitter-cli. None = not installed.
    fn check_twitter_cli(&self) -> Option<(String, String)> {
        let probe = probe_command("twitter", &["status"], 15, 1, Some("twitter-cli"));

        if probe.status == ProbeStatus::Missing {
            return None;
        }
        if probe.status == ProbeStatus::Broken {
            return Some((
                "error".to_string(),
                format!("twitter-cli 命令存在但无法执行。\n{}", probe.hint),
            ));
        }
        if probe.status == ProbeStatus::Timeout {
            return Some((
                "error".to_string(),
                format!("twitter-cli 健康检查超时（已重试 1 次）。\n{}", probe.hint),
            ));
        }

        let output = &probe.output;
        if output.contains("ok: true") {
            return Some((
                "ok".to_string(),
                concat!(
                    "twitter-cli 完整可用（搜索、读推文、时间线、长文/Article、",
                    "用户查询、Thread）"
                )
                .to_string(),
            ));
        }
        if output.contains("not_authenticated") {
            return Some((
                "warn".to_string(),
                concat!(
                    "twitter-cli 已安装但未认证。设置方式：\n",
                    "  export TWITTER_AUTH_TOKEN=\"xxx\"\n",
                    "  export TWITTER_CT0=\"yyy\"\n",
                    "或确保已在浏览器中登录 x.com"
                )
                .to_string(),
            ));
        }
        Some((
            "warn".to_string(),
            "twitter-cli 已安装但认证检查失败。运行：\n  twitter -v status 查看详细信息"
                .to_string(),
        ))
    }

    // ── OpenCLI probe ─────────────────────────────────────────────────

    /// OpenCLI candidate. None = not installed.
    fn check_opencli(&self) -> Option<(String, String)> {
        let st: OpenCLIStatus = opencli_status(15);
        if !st.installed {
            return None;
        }
        if st.broken {
            return Some(("error".to_string(), st.hint));
        }
        if st.ready() {
            return Some((
                "ok".to_string(),
                concat!(
                    "OpenCLI 可用（复用浏览器登录态）。用法：",
                    "opencli twitter search/article/user-posts -f yaml"
                )
                .to_string(),
            ));
        }
        Some(("warn".to_string(), st.hint))
    }
}

impl Default for TwitterChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl Channel for TwitterChannel {
    fn name(&self) -> &str {
        "twitter"
    }

    fn description(&self) -> &str {
        "Twitter/X 推文"
    }

    fn backends(&self) -> &[&str] {
        &["Twitter API (native)", "twitter-cli", "OpenCLI"]
    }

    fn tier(&self) -> u8 {
        1
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_lowercase();
                host.contains("x.com") || host.contains("twitter.com")
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
        let mut findings: Vec<(String, String, String)> = Vec::new(); // (backend, status, message)

        for backend in self.ordered_backends(config) {
            let result = if backend == "Twitter API (native)" {
                self.check_native_api(config)
            } else if backend == "twitter-cli" {
                self.check_twitter_cli()
            } else if backend == "OpenCLI" {
                self.check_opencli()
            } else {
                continue;
            };

            if let Some((status, msg)) = result {
                findings.push((backend, status, msg));
            }
        }

        // First fully-usable (ok) backend wins, then first fixable (warn)
        for wanted in &["ok", "warn"] {
            for (backend, status, message) in &findings {
                if status == *wanted {
                    self.active_backend = Some(backend.clone());
                    let status = CheckStatus::from_str(status);
                    return CheckResult {
                        status,
                        message: message.clone(),
                        active_backend: self.active_backend.clone(),
                    };
                }
            }
        }

        // Only broken/timeout candidates left
        if !findings.is_empty() {
            let messages: Vec<String> = findings.iter().map(|(_, _, m)| m.clone()).collect();
            return CheckResult {
                status: CheckStatus::Error,
                message: messages.join("\n"),
                active_backend: None,
            };
        }

        // Nothing installed at all
        CheckResult {
            status: CheckStatus::Warn,
            message: concat!(
                "Twitter 后端未配置。推荐方式：\n",
                "  1. 登录凭据（首选）：在 ~/.agent-reach/config.yaml 中配置\n",
                "     twitter_auth_token 和 twitter_ct0\n",
                "  2. twitter-cli（备选）：pipx install twitter-cli\n",
                "  3. OpenCLI（备选）：复用浏览器登录态"
            )
            .to_string(),
            active_backend: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_twitter() {
        let ch = TwitterChannel::new();
        assert!(ch.can_handle("https://x.com/user/status/123"));
        assert!(ch.can_handle("https://twitter.com/user/status/456"));
        assert!(ch.can_handle("https://www.x.com/search?q=rust"));
        assert!(!ch.can_handle("https://www.youtube.com/watch?v=abc"));
        assert!(!ch.can_handle("https://github.com/user/repo"));
        assert!(!ch.can_handle("not-a-url"));
    }

    #[test]
    fn test_name_and_tier() {
        let ch = TwitterChannel::new();
        assert_eq!(ch.name(), "twitter");
        assert_eq!(ch.description(), "Twitter/X 推文");
        assert_eq!(ch.tier(), 1);
    }

    #[test]
    fn test_backends_order() {
        let ch = TwitterChannel::new();
        let backends = ch.backends();
        assert_eq!(backends.len(), 3);
        assert_eq!(backends[0], "Twitter API (native)");
        assert_eq!(backends[1], "twitter-cli");
        assert_eq!(backends[2], "OpenCLI");
    }

    #[test]
    fn test_active_backend_get_set() {
        let mut ch = TwitterChannel::new();
        assert!(ch.active_backend().is_none());
        ch.set_active_backend(Some("Twitter API (native)".to_string()));
        assert_eq!(
            ch.active_backend(),
            Some("Twitter API (native)".to_string())
        );
        ch.set_active_backend(None);
        assert!(ch.active_backend().is_none());
    }
}

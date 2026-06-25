//! Channel registry — lists all supported platforms for doctor checks.

use crate::config::Config;
use crate::doctor::ChannelResult;
use std::collections::HashMap;

pub mod base;
pub mod bilibili;
pub mod exa_search;
pub mod github;
pub mod linkedin;
pub mod reddit;
pub mod rss;
pub mod twitter;
pub mod v2ex;
pub mod web;
pub mod xiaohongshu;
pub mod xiaoyuzhou;
pub mod xueqiu;
pub mod youtube;

pub use base::{Channel, CheckResult, CheckStatus};

use bilibili::BilibiliChannel;
use exa_search::ExaSearchChannel;
use github::GitHubChannel;
use linkedin::LinkedInChannel;
use reddit::RedditChannel;
use rss::RSSChannel;
use twitter::TwitterChannel;
use v2ex::V2EXChannel;
use web::WebChannel;
use xiaohongshu::XiaoHongShuChannel;
use xiaoyuzhou::XiaoyuzhouChannel;
use xueqiu::XueqiuChannel;
use youtube::YouTubeChannel;

/// Get all registered channels.
pub fn get_all_channels() -> Vec<Box<dyn Channel>> {
    vec![
        Box::new(GitHubChannel::new()),
        Box::new(TwitterChannel::new()),
        Box::new(YouTubeChannel::new()),
        Box::new(RedditChannel::new()),
        Box::new(BilibiliChannel::new()),
        Box::new(XiaoHongShuChannel::new()),
        Box::new(LinkedInChannel::new()),
        Box::new(XiaoyuzhouChannel::new()),
        Box::new(V2EXChannel::new()),
        Box::new(XueqiuChannel::new()),
        Box::new(RSSChannel::new()),
        Box::new(ExaSearchChannel::new()),
        Box::new(WebChannel::new()),
    ]
}

/// Get a channel by name.
pub fn get_channel(name: &str) -> Option<Box<dyn Channel>> {
    get_all_channels().into_iter().find(|ch| ch.name() == name)
}

/// Check all channels and return results.
pub fn check_all(config: &Config) -> HashMap<String, ChannelResult> {
    let mut results = HashMap::new();
    let mut channels = get_all_channels();

    for ch in &mut channels {
        let name = ch.name().to_string();
        let description = ch.description().to_string();
        let tier = ch.tier();
        let backends = ch.backends().iter().map(|s| s.to_string()).collect();

        let (status, message, active_backend) = match ch.check(Some(config)) {
            cr @ CheckResult { .. } => {
                let ab = cr.active_backend.clone();
                (cr.status.as_str().to_string(), cr.message, ab)
            }
        };

        results.insert(
            name,
            ChannelResult {
                status,
                name: description,
                message,
                tier,
                backends,
                active_backend,
            },
        );
    }

    results
}

//! V2EX — public API channel for topics, nodes, users, and replies.

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;

const UA: &str = "agent-reach/1.0";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Helper: fetch *url* with agent-reach UA and 10 s timeout, return parsed JSON.
fn get_json(url: &str) -> Result<serde_json::Value, String> {
    let resp = ureq::get(url)
        .set("User-Agent", UA)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("HTTP 请求失败：{}", e))?;
    resp.into_json::<serde_json::Value>()
        .map_err(|e| format!("JSON 解析失败：{}", e))
}

pub struct V2EXChannel {
    active_backend: Option<String>,
}

impl V2EXChannel {
    pub fn new() -> Self {
        V2EXChannel {
            active_backend: None,
        }
    }

    // ------------------------------------------------------------------ //
    // Data-fetching methods (public, NOT trait methods)
    // ------------------------------------------------------------------ //

    /// Get hot topics list.
    ///
    /// Returns a Vec of JSON objects with keys:
    ///   id, title, url, replies, node_name, node_title, content, created
    pub fn get_hot_topics(&self, limit: usize) -> Result<Vec<serde_json::Value>, String> {
        let data = get_json("https://www.v2ex.com/api/topics/hot.json")?;

        let arr = data.as_array().ok_or("返回数据不是数组")?;
        let results: Vec<serde_json::Value> = arr
            .iter()
            .take(limit)
            .map(|item| {
                let node = item.get("node").and_then(|n| n.as_object());
                let content = item
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                serde_json::json!({
                    "id": item.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
                    "title": item.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    "url": item.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                    "replies": item.get("replies").and_then(|v| v.as_u64()).unwrap_or(0),
                    "node_name": node.and_then(|n| n.get("name")).and_then(|v| v.as_str()).unwrap_or(""),
                    "node_title": node.and_then(|n| n.get("title")).and_then(|v| v.as_str()).unwrap_or(""),
                    "content": &content[..content.len().min(200)],
                    "created": item.get("created").and_then(|v| v.as_u64()).unwrap_or(0),
                })
            })
            .collect();

        Ok(results)
    }

    /// Get latest topics from a specific node.
    ///
    /// Args:
    ///   node_name: node name, e.g. "python", "tech", "jobs"
    ///   limit: max number of results to return
    ///
    /// Returns a Vec of JSON objects with keys:
    ///   id, title, url, replies, node_name, node_title, content, created
    pub fn get_node_topics(
        &self,
        node_name: &str,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>, String> {
        let url = format!(
            "https://www.v2ex.com/api/topics/show.json?node_name={}&page=1",
            node_name
        );
        let data = get_json(&url)?;

        let arr = data.as_array().ok_or("返回数据不是数组")?;
        let results: Vec<serde_json::Value> = arr
            .iter()
            .take(limit)
            .map(|item| {
                let node = item.get("node").and_then(|n| n.as_object());
                let content = item
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                serde_json::json!({
                    "id": item.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
                    "title": item.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    "url": item.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                    "replies": item.get("replies").and_then(|v| v.as_u64()).unwrap_or(0),
                    "node_name": node.and_then(|n| n.get("name")).and_then(|v| v.as_str()).unwrap_or(node_name),
                    "node_title": node.and_then(|n| n.get("title")).and_then(|v| v.as_str()).unwrap_or(""),
                    "content": &content[..content.len().min(200)],
                    "created": item.get("created").and_then(|v| v.as_u64()).unwrap_or(0),
                })
            })
            .collect();

        Ok(results)
    }

    /// Get a single topic with its replies.
    ///
    /// Args:
    ///   topic_id: topic ID (from URL https://www.v2ex.com/t/<id>)
    ///
    /// Returns a JSON object with keys:
    ///   id, title, url, content, replies_count, node_name, node_title,
    ///   author, created, replies (Vec of {author, content, created})
    pub fn get_topic(&self, topic_id: u64) -> Result<serde_json::Value, String> {
        let topic_url = format!(
            "https://www.v2ex.com/api/topics/show.json?id={}",
            topic_id
        );
        let topic_data = get_json(&topic_url)?;

        // API returns a list even for single-ID queries
        let topic = if let Some(arr) = topic_data.as_array() {
            arr.first().cloned().unwrap_or(serde_json::Value::Null)
        } else {
            topic_data
        };

        let node = topic.get("node").and_then(|n| n.as_object());
        let member = topic.get("member").and_then(|m| m.as_object());

        // Fetch replies (first page)
        let replies_raw = get_json(&format!(
            "https://www.v2ex.com/api/replies/show.json?topic_id={}&page=1",
            topic_id
        ))
        .ok();

        let replies: Vec<serde_json::Value> = replies_raw
            .as_ref()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|r| {
                        serde_json::json!({
                            "author": r.get("member")
                                .and_then(|m| m.get("username"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(""),
                            "content": r.get("content").and_then(|v| v.as_str()).unwrap_or(""),
                            "created": r.get("created").and_then(|v| v.as_u64()).unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(serde_json::json!({
            "id": topic.get("id").and_then(|v| v.as_u64()).unwrap_or(topic_id),
            "title": topic.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            "url": topic.get("url").and_then(|v| v.as_str())
                .unwrap_or(&format!("https://www.v2ex.com/t/{}", topic_id)),
            "content": topic.get("content").and_then(|v| v.as_str()).unwrap_or(""),
            "replies_count": topic.get("replies").and_then(|v| v.as_u64()).unwrap_or(0),
            "node_name": node.and_then(|n| n.get("name")).and_then(|v| v.as_str()).unwrap_or(""),
            "node_title": node.and_then(|n| n.get("title")).and_then(|v| v.as_str()).unwrap_or(""),
            "author": member.and_then(|m| m.get("username")).and_then(|v| v.as_str()).unwrap_or(""),
            "created": topic.get("created").and_then(|v| v.as_u64()).unwrap_or(0),
            "replies": replies,
        }))
    }

    /// Get user profile information.
    ///
    /// Args:
    ///   username: V2EX username
    ///
    /// Returns a JSON object with keys:
    ///   id, username, url, website, twitter, psn, github, btc,
    ///   location, bio, avatar, created
    pub fn get_user(&self, username: &str) -> Result<serde_json::Value, String> {
        let url = format!(
            "https://www.v2ex.com/api/members/show.json?username={}",
            username
        );
        let data = get_json(&url)?;

        Ok(serde_json::json!({
            "id": data.get("id").and_then(|v| v.as_u64()).unwrap_or(0),
            "username": data.get("username").and_then(|v| v.as_str()).unwrap_or(username),
            "url": data.get("url").and_then(|v| v.as_str())
                .unwrap_or(&format!("https://www.v2ex.com/member/{}", username)),
            "website": data.get("website").and_then(|v| v.as_str()).unwrap_or(""),
            "twitter": data.get("twitter").and_then(|v| v.as_str()).unwrap_or(""),
            "psn": data.get("psn").and_then(|v| v.as_str()).unwrap_or(""),
            "github": data.get("github").and_then(|v| v.as_str()).unwrap_or(""),
            "btc": data.get("btc").and_then(|v| v.as_str()).unwrap_or(""),
            "location": data.get("location").and_then(|v| v.as_str()).unwrap_or(""),
            "bio": data.get("bio").and_then(|v| v.as_str()).unwrap_or(""),
            "avatar": data.get("avatar_large").and_then(|v| v.as_str())
                .or_else(|| data.get("avatar_normal").and_then(|v| v.as_str()))
                .unwrap_or(""),
            "created": data.get("created").and_then(|v| v.as_u64()).unwrap_or(0),
        }))
    }

    /// Search topics.
    ///
    /// Note: V2EX public API does not provide a search endpoint
    /// (/api/search.json is unavailable). This method returns an error
    /// message suggesting alternatives.
    ///
    /// Returns:
    ///   Vec of JSON objects; on failure, contains a single {"error": "…"} entry.
    pub fn search(&self, query: &str, _limit: usize) -> Vec<serde_json::Value> {
        vec![serde_json::json!({
            "error": format!(
                "V2EX 公开 API 不提供搜索端点。\
                 建议改用：https://www.v2ex.com/?q={} \
                 或通过 Exa channel 使用 site:v2ex.com 搜索。",
                query
            )
        })]
    }
}

impl Channel for V2EXChannel {
    fn name(&self) -> &str {
        "v2ex"
    }

    fn description(&self) -> &str {
        "V2EX 节点、主题与回复"
    }

    fn backends(&self) -> &[&str] {
        &["V2EX API (public)"]
    }

    fn tier(&self) -> u8 {
        0
    }

    fn can_handle(&self, url_str: &str) -> bool {
        match url::Url::parse(url_str) {
            Ok(u) => u
                .host_str()
                .unwrap_or("")
                .to_lowercase()
                .contains("v2ex.com"),
            Err(_) => false,
        }
    }

    fn active_backend(&self) -> Option<String> {
        self.active_backend.clone()
    }

    fn set_active_backend(&mut self, backend: Option<String>) {
        self.active_backend = backend;
    }

    fn check(&mut self, _config: Option<&Config>) -> CheckResult {
        match get_json(
            "https://www.v2ex.com/api/topics/show.json?node_name=python&page=1",
        ) {
            Ok(_) => {
                self.active_backend = Some("V2EX API (public)".to_string());
                CheckResult {
                    status: CheckStatus::Ok,
                    message: "公开 API 可用（热门主题、节点浏览、主题详情、用户信息）"
                        .to_string(),
                    active_backend: self.active_backend.clone(),
                }
            }
            Err(e) => {
                self.active_backend = None;
                CheckResult {
                    status: CheckStatus::Warn,
                    message: format!("V2EX API 连接失败（可能需要代理）：{}", e),
                    active_backend: None,
                }
            }
        }
    }
}

//! XiaoHongShu (小红书) — multi-backend: OpenCLI / xiaohongshu-mcp / xhs-cli.
//!
//! Backend order encodes the recommendation, and probing order makes the
//! environment split automatic: OpenCLI needs a desktop Chrome so it simply
//! never probes alive on a server, where xiaohongshu-mcp (self-contained
//! headless browser) takes over. xhs-cli (upstream unmaintained since
//! 2026-03) keeps working for existing installs as the last candidate.

use serde_json::Value;
use url::Url;

use crate::backends::{opencli_status, OpenCLIStatus};
use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;
use crate::probe::{probe_command, ProbeStatus};

const MCP_ENDPOINT: &str = "http://localhost:18060/mcp";
const MCP_INSTALL_URL: &str = "https://github.com/xpzouying/xiaohongshu-mcp";

/// XiaoHongShu channel — multi-backend with OpenCLI, xiaohongshu-mcp, and xhs-cli.
pub struct XiaoHongShuChannel {
    pub active_backend: Option<String>,
}

impl XiaoHongShuChannel {
    pub fn new() -> Self {
        XiaoHongShuChannel {
            active_backend: None,
        }
    }

    // ── backend probes ──────────────────────────────────────────────

    /// OpenCLI candidate. None = not installed.
    fn check_opencli(&self) -> Option<(String, String)> {
        let st: OpenCLIStatus = opencli_status(10);
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
                    "opencli xiaohongshu search/note/comments/feed -f yaml"
                )
                .to_string(),
            ));
        }
        Some(("warn".to_string(), st.hint))
    }

    /// xiaohongshu-mcp candidate. None = service not running / not reachable.
    fn check_mcp(&self) -> Option<(String, String)> {
        if !mcp_service_reachable() {
            return None;
        }
        let mcporter = probe_command(
            "mcporter",
            &["config", "list"],
            10,
            0,
            Some("mcporter"),
        );
        if mcporter.ok() && mcporter.output.contains("xiaohongshu") {
            return Some((
                "ok".to_string(),
                concat!(
                    "xiaohongshu-mcp 服务运行中",
                    "（mcporter call 'xiaohongshu.search_feeds(keyword: \"...\")'）。",
                    "若未登录，让 agent 调 get_login_qrcode 扫码"
                )
                .to_string(),
            ));
        }
        Some((
            "warn".to_string(),
            format!(
                "xiaohongshu-mcp 服务在跑但 mcporter 未接入。运行：\n  mcporter config add xiaohongshu {}",
                MCP_ENDPOINT
            ),
        ))
    }

    /// Legacy xhs-cli candidate. None = not installed.
    fn check_xhs_cli(&self) -> Option<(String, String)> {
        let probe = probe_command(
            "xhs",
            &["status"],
            10,
            0,
            Some("xiaohongshu-cli"),
        );
        if probe.status == ProbeStatus::Missing {
            return None;
        }
        if probe.status == ProbeStatus::Broken {
            return Some((
                "error".to_string(),
                format!("xhs 命令存在但无法执行\n{}", probe.hint),
            ));
        }
        if probe.status == ProbeStatus::Timeout {
            return Some((
                "warn".to_string(),
                format!("xhs-cli 已安装但状态检测超时\n{}", probe.hint),
            ));
        }

        // Process is alive (ran successfully or non-zero exit) — classify by output
        if probe.ok() && probe.output.contains("ok: true") {
            return Some((
                "ok".to_string(),
                concat!(
                    "xhs-cli 可用（搜索、阅读、评论、热门；上游 2026-03 起停更，",
                    "桌面用户建议迁移到 OpenCLI）"
                )
                .to_string(),
            ));
        }
        if probe.output.contains("not_authenticated") || probe.output.contains("expired") {
            return Some((
                "warn".to_string(),
                concat!(
                    "xhs-cli 已安装但未登录。运行：\n",
                    "  xhs login\n",
                    "（自动从浏览器提取 Cookie，或扫码登录）"
                )
                .to_string(),
            ));
        }
        Some((
            "warn".to_string(),
            "xhs-cli 已安装但状态异常。运行：\n  xhs -v status 查看详细信息".to_string(),
        ))
    }
}

impl Channel for XiaoHongShuChannel {
    fn name(&self) -> &str {
        "xiaohongshu"
    }

    fn description(&self) -> &str {
        "小红书笔记"
    }

    fn backends(&self) -> &[&str] {
        &["OpenCLI", "xiaohongshu-mcp", "xhs-cli (xiaohongshu-cli)"]
    }

    fn tier(&self) -> u8 {
        1
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("").to_lowercase();
                host.contains("xiaohongshu.com") || host.contains("xhslink.com")
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
            let result = if backend == "OpenCLI" {
                self.check_opencli()
            } else if backend == "xiaohongshu-mcp" {
                self.check_mcp()
            } else {
                self.check_xhs_cli()
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

        // Only broken candidates left
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
            status: CheckStatus::Off,
            message: format!(
                "未安装任何小红书后端。推荐：\n\
                  桌面：agent-reach install --channels opencli\n\
                 \x20      （复用 Chrome 登录态，刷过小红书即零配置可用）\n\
                  服务器：xiaohongshu-mcp（自带无头浏览器+扫码登录）：{}",
                MCP_INSTALL_URL
            ),
            active_backend: None,
        }
    }
}

// ── HTTP helper ────────────────────────────────────────────────────

/// True if the xiaohongshu-mcp HTTP service answers on localhost.
///
/// Any HTTP response counts (the MCP endpoint replies 405 to GET) —
/// we only care that the service is up. Proxies are bypassed explicitly:
/// localhost must never be routed through HTTP_PROXY.
fn mcp_service_reachable() -> bool {
    // Build an agent that explicitly bypasses proxies for localhost.
    // ureq's global agent may respect HTTP_PROXY; we construct a fresh
    // agent with no proxy configured.
    let agent = ureq::AgentBuilder::new().build();
    match agent.get(MCP_ENDPOINT).call() {
        Ok(_) => true,
        Err(ureq::Error::Status(_, _)) => true, // 405/404 etc. — service is alive
        Err(_) => false,
    }
}

// ── data formatting ────────────────────────────────────────────────

/// Clean XHS API response, keeping only useful fields.
///
/// Handles both single note objects and lists of notes (search results).
/// Drastically reduces token usage by stripping structural redundancy.
pub fn format_xhs_result(data: &Value) -> Value {
    match data {
        Value::Array(arr) => {
            let cleaned: Vec<Value> = arr.iter().map(clean_note).collect();
            Value::Array(cleaned)
        }
        Value::Object(_) => {
            // Handle search_feeds wrapper: {"items": [...]} or {"data": {"items": [...]}}
            let items = data
                .get("items")
                .or_else(|| {
                    data.get("data")
                        .and_then(|d| d.get("items").or_else(|| d.get("notes")))
                });
            if let Some(Value::Array(items)) = items {
                let cleaned: Vec<Value> = items.iter().map(clean_note).collect();
                return Value::Array(cleaned);
            }
            // Single note
            clean_note(data)
        }
        _ => data.clone(),
    }
}

/// Extract useful fields from a single XHS note/feed item.
fn clean_note(note: &Value) -> Value {
    let obj = match note {
        Value::Object(_) => note,
        _ => return note.clone(),
    };

    // Some responses nest the note under "note_card" or "note"
    let inner = obj
        .get("note_card")
        .or_else(|| obj.get("note"))
        .unwrap_or(obj);

    let inner_obj = match inner {
        Value::Object(o) => o,
        _ => return note.clone(),
    };

    let mut result = serde_json::Map::new();

    // Basic info
    for key in &["id", "note_id", "xsec_token", "title", "desc", "type", "time"] {
        if let Some(val) = inner_obj.get(*key) {
            result.insert(key.to_string(), val.clone());
        }
    }

    // Content (may be in desc or content)
    if let Some(content) = inner_obj.get("content") {
        if !result.contains_key("desc") {
            result.insert("content".to_string(), content.clone());
        }
    }

    // Author
    let user = inner_obj
        .get("user")
        .or_else(|| inner_obj.get("author"));
    if let Some(Value::Object(user_obj)) = user {
        let mut clean_user = serde_json::Map::new();
        for key in &["nickname", "user_id", "nick_name"] {
            if let Some(val) = user_obj.get(*key) {
                clean_user.insert(key.to_string(), val.clone());
            }
        }
        if !clean_user.is_empty() {
            result.insert("user".to_string(), Value::Object(clean_user));
        }
    }

    // Engagement metrics — try nested then top-level
    let interact = inner_obj
        .get("interact_info")
        .or_else(|| inner_obj.get("note_interact_info"))
        .and_then(|v| v.as_object());
    if let Some(interact_obj) = interact {
        for key in &["liked_count", "collected_count", "comment_count", "share_count"] {
            if let Some(val) = interact_obj.get(*key) {
                result.insert(key.to_string(), val.clone());
            }
        }
    }
    for key in &["liked_count", "collected_count", "comment_count", "share_count"] {
        if let Some(val) = inner_obj.get(*key) {
            result.entry(key.to_string()).or_insert_with(|| val.clone());
        }
    }

    // Images — just URLs
    let images = inner_obj
        .get("image_list")
        .or_else(|| inner_obj.get("images_list"))
        .and_then(|v| v.as_array());
    if let Some(images_arr) = images {
        let mut urls: Vec<Value> = Vec::new();
        for img in images_arr {
            if let Value::Object(img_obj) = img {
                let url = img_obj
                    .get("url")
                    .or_else(|| img_obj.get("url_default"))
                    .or_else(|| img_obj.get("original"));
                if let Some(u) = url {
                    urls.push(u.clone());
                }
            } else if img.is_string() {
                urls.push(img.clone());
            }
        }
        if !urls.is_empty() {
            result.insert("images".to_string(), Value::Array(urls));
        }
    }

    // Tags
    let tags = inner_obj
        .get("tag_list")
        .or_else(|| inner_obj.get("tags"))
        .and_then(|v| v.as_array());
    if let Some(tags_arr) = tags {
        let mut tag_names: Vec<Value> = Vec::new();
        for t in tags_arr {
            if let Value::Object(t_obj) = t {
                if let Some(name) = t_obj.get("name") {
                    tag_names.push(name.clone());
                }
            } else if t.is_string() {
                tag_names.push(t.clone());
            }
        }
        if !tag_names.is_empty() {
            result.insert("tags".to_string(), Value::Array(tag_names));
        }
    }

    // Comments (if present, e.g. from get_feed_detail with comments)
    let comments = inner_obj
        .get("comments")
        .and_then(|v| v.as_array());
    if let Some(comments_arr) = comments {
        if !comments_arr.is_empty() {
            let cleaned: Vec<Value> = comments_arr.iter().map(clean_comment).collect();
            result.insert("comments".to_string(), Value::Array(cleaned));
        }
    }

    Value::Object(result)
}

/// Extract useful fields from a comment.
fn clean_comment(comment: &Value) -> Value {
    let obj = match comment {
        Value::Object(_) => comment,
        _ => return comment.clone(),
    };
    let comment_obj = match obj.as_object() {
        Some(o) => o,
        None => return comment.clone(),
    };

    let mut result = serde_json::Map::new();

    if let Some(content) = comment_obj.get("content") {
        result.insert("content".to_string(), content.clone());
    }

    let user = comment_obj
        .get("user_info")
        .or_else(|| comment_obj.get("user"));
    if let Some(Value::Object(user_obj)) = user {
        let nickname = user_obj
            .get("nickname")
            .or_else(|| user_obj.get("nick_name"));
        if let Some(n) = nickname {
            result.insert("user".to_string(), n.clone());
        }
    }

    for key in &["like_count", "sub_comment_count"] {
        if let Some(val) = comment_obj.get(*key) {
            result.insert(key.to_string(), val.clone());
        }
    }

    Value::Object(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_handle_xiaohongshu() {
        let ch = XiaoHongShuChannel::new();
        assert!(ch.can_handle("https://www.xiaohongshu.com/explore/abc123"));
        assert!(ch.can_handle("http://xhslink.com/abc"));
        assert!(!ch.can_handle("https://www.youtube.com/watch?v=abc"));
        assert!(!ch.can_handle("https://github.com/user/repo"));
    }

    #[test]
    fn test_format_single_note() {
        let data = serde_json::json!({
            "id": "123",
            "title": "Hello小红书",
            "desc": "测试笔记",
            "type": "normal",
            "time": 1719216000,
            "user": {
                "nickname": "测试用户",
                "user_id": "user123"
            },
            "interact_info": {
                "liked_count": 100,
                "collected_count": 50
            },
            "image_list": [
                {"url": "https://example.com/img1.jpg"},
                {"url_default": "https://example.com/img2.jpg"}
            ],
            "tag_list": [
                {"name": "美食"},
                {"name": "旅行"}
            ]
        });

        let result = format_xhs_result(&data);
        let obj = result.as_object().unwrap();

        assert_eq!(obj.get("id").unwrap().as_str().unwrap(), "123");
        assert_eq!(
            obj.get("title").unwrap().as_str().unwrap(),
            "Hello小红书"
        );
        assert_eq!(obj.get("desc").unwrap().as_str().unwrap(), "测试笔记");

        let user = obj.get("user").unwrap().as_object().unwrap();
        assert_eq!(user.get("nickname").unwrap().as_str().unwrap(), "测试用户");

        assert_eq!(obj.get("liked_count").unwrap().as_u64().unwrap(), 100);
        assert_eq!(obj.get("collected_count").unwrap().as_u64().unwrap(), 50);

        let images = obj.get("images").unwrap().as_array().unwrap();
        assert_eq!(images.len(), 2);

        let tags = obj.get("tags").unwrap().as_array().unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[test]
    fn test_format_note_list() {
        let data = serde_json::json!([
            {"id": "1", "title": "Note 1", "type": "normal"},
            {"id": "2", "title": "Note 2", "type": "video"}
        ]);

        let result = format_xhs_result(&data);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_object().unwrap().get("id").unwrap().as_str().unwrap(), "1");
        assert_eq!(arr[1].as_object().unwrap().get("id").unwrap().as_str().unwrap(), "2");
    }

    #[test]
    fn test_format_search_feeds_wrapper() {
        let data = serde_json::json!({
            "items": [
                {"id": "1", "title": "Note 1"},
                {"id": "2", "title": "Note 2"}
            ]
        });

        let result = format_xhs_result(&data);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_format_data_items_wrapper() {
        let data = serde_json::json!({
            "data": {
                "items": [
                    {"id": "3", "title": "Note 3"}
                ]
            }
        });

        let result = format_xhs_result(&data);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_object().unwrap().get("id").unwrap().as_str().unwrap(), "3");
    }

    #[test]
    fn test_format_with_comments() {
        let data = serde_json::json!({
            "id": "1",
            "title": "Note with comments",
            "comments": [
                {
                    "content": "好棒！",
                    "user_info": {"nickname": "评论者"},
                    "like_count": 5,
                    "sub_comment_count": 2
                }
            ]
        });

        let result = format_xhs_result(&data);
        let obj = result.as_object().unwrap();
        let comments = obj.get("comments").unwrap().as_array().unwrap();
        let c = comments[0].as_object().unwrap();
        assert_eq!(c.get("content").unwrap().as_str().unwrap(), "好棒！");
        assert_eq!(c.get("user").unwrap().as_str().unwrap(), "评论者");
        assert_eq!(c.get("like_count").unwrap().as_u64().unwrap(), 5);
    }

    #[test]
    fn test_format_non_dict_passthrough() {
        let data = serde_json::json!("just a string");
        let result = format_xhs_result(&data);
        assert_eq!(result.as_str().unwrap(), "just a string");
    }

    #[test]
    fn test_note_card_nesting() {
        let data = serde_json::json!({
            "note_card": {
                "id": "inner123",
                "title": "Inner title",
                "user": {"nickname": "Inner User"}
            }
        });

        let result = format_xhs_result(&data);
        let obj = result.as_object().unwrap();
        assert_eq!(obj.get("id").unwrap().as_str().unwrap(), "inner123");
        assert_eq!(
            obj.get("title").unwrap().as_str().unwrap(),
            "Inner title"
        );
    }
}

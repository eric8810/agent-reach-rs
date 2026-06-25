//! Xueqiu (雪球) channel — stock quotes, search, trending posts & hot stocks.
//!
//! Provides:
//! - `get_stock_quote`  — real-time quote for a given symbol
//! - `search_stock`     — search stocks by code or Chinese name
//! - `get_hot_posts`    — trending community posts
//! - `get_hot_stocks`   — popular stocks ranking
//!
//! Cookies are loaded from config key `xueqiu_cookie` (a
//! `"name=value; name2=value2"` string).  When that key is absent a
//! one-off homepage visit picks up the anti-DDoS `acw_tc` cookie so
//! publicly-accessible endpoints still work.

use std::collections::HashMap;
use std::time::Duration;

use url::Url;

use crate::channels::base::{Channel, CheckResult, CheckStatus};
use crate::config::Config;

// ── constants ────────────────────────────────────────────────────────

const UA: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const REFERER: &str = "https://xueqiu.com/";
const TIMEOUT_SECS: u64 = 10;

// ── helpers ──────────────────────────────────────────────────────────

/// Remove HTML tags and decode common entities.
fn strip_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .trim()
        .to_string()
}

// ── XueqiuChannel ────────────────────────────────────────────────────

pub struct XueqiuChannel {
    active_backend: Option<String>,
    /// Cookie store: name → value.
    cookies: HashMap<String, String>,
    /// True after the first call to `ensure_cookies` (avoids repeated
    /// homepage hits when no config key is set).
    cookies_attempted: bool,
}

impl XueqiuChannel {
    pub fn new() -> Self {
        XueqiuChannel {
            active_backend: None,
            cookies: HashMap::new(),
            cookies_attempted: false,
        }
    }

    // ── cookie loading ──────────────────────────────────────────

    /// Parse `"name=value; name2=value2"` and insert into the store.
    fn inject_cookie_string(&mut self, raw: &str) {
        for pair in raw.split(';') {
            let pair = pair.trim();
            if let Some((name, value)) = pair.split_once('=') {
                self.cookies
                    .insert(name.trim().to_string(), value.trim().to_string());
            }
        }
    }

    /// Try the config key `xueqiu_cookie`.  Returns true when cookies
    /// were loaded.
    fn load_from_config(&mut self, config: Option<&Config>) -> bool {
        let cfg = match config {
            Some(c) => c,
            None => return false,
        };
        match cfg.get("xueqiu_cookie") {
            Some(s) if !s.is_empty() => {
                self.inject_cookie_string(&s);
                true
            }
            _ => false,
        }
    }

    /// Fallback: hit the homepage so the server sends back the
    /// anti-DDoS `acw_tc` cookie.  Extract `Set-Cookie` headers from
    /// the response.
    fn load_from_homepage(&mut self) -> bool {
        let resp = match ureq::get("https://xueqiu.com/")
            .set("User-Agent", UA)
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .call()
        {
            Ok(r) => r,
            Err(_) => return false,
        };

        let headers = resp.all("Set-Cookie");
        for h in &headers {
            // Set-Cookie: name=value; Path=/; ...
            if let Some(pair) = h.split(';').next() {
                if let Some((name, value)) = pair.split_once('=') {
                    self.cookies
                        .entry(name.trim().to_string())
                        .or_insert_with(|| value.trim().to_string());
                }
            }
        }
        !headers.is_empty()
    }

    /// Make sure the cookie store is populated.
    ///
    /// Priority: ① config key `xueqiu_cookie`  ② homepage visit.
    fn ensure_cookies(&mut self, config: Option<&Config>) {
        if self.cookies_attempted {
            return;
        }
        self.cookies_attempted = true;

        if self.load_from_config(config) {
            return;
        }
        self.load_from_homepage();
    }

    /// Serialise stored cookies into a `Cookie:` header value.
    fn cookie_header_value(&self) -> Option<String> {
        if self.cookies.is_empty() {
            return None;
        }
        let s: String = self
            .cookies
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ");
        Some(s)
    }

    // ── HTTP helper ──────────────────────────────────────────────

    /// GET *url* with Xueqiu session cookies and return parsed JSON.
    fn get_json(
        &mut self,
        url: &str,
        config: Option<&Config>,
    ) -> Result<serde_json::Value, String> {
        self.ensure_cookies(config);

        let mut req = ureq::get(url)
            .set("User-Agent", UA)
            .set("Referer", REFERER);

        if let Some(ref cookie_str) = self.cookie_header_value() {
            req = req.set("Cookie", cookie_str);
        }

        let resp = req
            .timeout(Duration::from_secs(TIMEOUT_SECS))
            .call()
            .map_err(|e| format!("HTTP 请求失败：{e}"))?;
        let body = resp
            .into_string()
            .map_err(|e| format!("读取响应失败：{e}"))?;

        serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败：{e}"))
    }

    // ── public data-fetching methods ─────────────────────────────

    /// Get real-time stock quote.
    ///
    /// `symbol` examples: `SH600519` (沪), `SZ000858` (深), `AAPL` (美),
    /// `00700` (港).
    ///
    /// Returns a map with keys: symbol, name, current, percent, chg,
    /// high, low, open, last_close, volume, amount, market_capital,
    /// turnover_rate, pe_ttm, timestamp.
    pub fn get_stock_quote(
        &mut self,
        symbol: &str,
        config: Option<&Config>,
    ) -> Result<HashMap<String, serde_json::Value>, String> {
        let url = format!(
            "https://stock.xueqiu.com/v5/stock/batch/quote.json?symbol={symbol}"
        );
        let data = self.get_json(&url, config)?;

        let items: Vec<serde_json::Value> = data
            .get("data")
            .and_then(|d| d.get("items"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let quote = items
            .first()
            .and_then(|item| item.get("quote"))
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        let fields: &[&str] = &[
            "symbol",
            "name",
            "current",
            "percent",
            "chg",
            "high",
            "low",
            "open",
            "last_close",
            "volume",
            "amount",
            "market_capital",
            "turnover_rate",
            "pe_ttm",
            "timestamp",
        ];

        let mut result = HashMap::new();
        for field in fields {
            if let Some(val) = quote.get(field) {
                result.insert(field.to_string(), val.clone());
            }
        }
        // Always include the symbol key even when the API omits it.
        result
            .entry("symbol".to_string())
            .or_insert(serde_json::Value::String(symbol.to_string()));

        Ok(result)
    }

    /// Search stocks by code or Chinese name.
    ///
    /// Returns a list of maps with keys: `symbol`, `name`, `exchange`.
    pub fn search_stock(
        &mut self,
        query: &str,
        limit: usize,
        config: Option<&Config>,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, String> {
        let encoded: String =
            url::form_urlencoded::byte_serialize(query.as_bytes()).collect();
        let url = format!(
            "https://xueqiu.com/stock/search.json?code={encoded}&size={limit}"
        );
        let data = self.get_json(&url, config)?;

        let stocks: Vec<serde_json::Value> = data
            .get("stocks")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(stocks
            .into_iter()
            .take(limit)
            .map(|s| {
                let mut m = HashMap::new();
                m.insert(
                    "symbol".to_string(),
                    s.get("code")
                        .cloned()
                        .unwrap_or(serde_json::Value::String(String::new())),
                );
                m.insert(
                    "name".to_string(),
                    s.get("name")
                        .cloned()
                        .unwrap_or(serde_json::Value::String(String::new())),
                );
                m.insert(
                    "exchange".to_string(),
                    s.get("exchange")
                        .cloned()
                        .unwrap_or(serde_json::Value::String(String::new())),
                );
                m
            })
            .collect())
    }

    /// Get trending posts from the Xueqiu community.
    ///
    /// Each item's `data` field is a JSON-encoded string containing the
    /// actual post payload (title, description, user, like_count, target).
    ///
    /// Returns a list of maps with keys: `id`, `title`, `text`, `author`,
    /// `likes`, `url`.
    pub fn get_hot_posts(
        &mut self,
        limit: usize,
        config: Option<&Config>,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, String> {
        let url = concat!(
            "https://xueqiu.com/v4/statuses/public_timeline_by_category.json",
            "?since_id=-1&max_id=-1&count=20&category=-1"
        );
        let data = self.get_json(url, config)?;

        let items: Vec<serde_json::Value> = data
            .get("list")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(items
            .into_iter()
            .take(limit)
            .filter_map(|item| {
                // `data` is a JSON-encoded string containing the real post.
                let data_str = item.get("data").and_then(|v| v.as_str()).unwrap_or("");
                let post: serde_json::Value =
                    serde_json::from_str(data_str).unwrap_or(serde_json::Value::Null);

                let user = post
                    .get("user")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);

                let raw_text = post
                    .get("text")
                    .or_else(|| post.get("description"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let text = strip_html(raw_text);
                let text_trunc: String = text.chars().take(200).collect();

                let target = post
                    .get("target")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let mut m = HashMap::new();
                m.insert(
                    "id".to_string(),
                    post.get("id")
                        .cloned()
                        .unwrap_or(serde_json::Value::Number(0.into())),
                );
                m.insert("title".to_string(), post.get("title").cloned().unwrap_or_default());
                m.insert(
                    "text".to_string(),
                    serde_json::Value::String(text_trunc),
                );
                m.insert(
                    "author".to_string(),
                    user.get("screen_name").cloned().unwrap_or_default(),
                );
                m.insert(
                    "likes".to_string(),
                    post.get("like_count")
                        .cloned()
                        .unwrap_or(serde_json::Value::Number(0.into())),
                );
                m.insert(
                    "url".to_string(),
                    if target.is_empty() {
                        serde_json::Value::String(String::new())
                    } else {
                        serde_json::Value::String(format!("https://xueqiu.com{target}"))
                    },
                );
                Some(m)
            })
            .collect())
    }

    /// Get hot stocks ranking.
    ///
    /// `stock_type`: `10` = popularity ranking (default), `12` = watchlist.
    ///
    /// Returns a list of maps with keys: `symbol`, `name`, `current`,
    /// `percent`, `rank`.
    pub fn get_hot_stocks(
        &mut self,
        limit: usize,
        stock_type: u8,
        config: Option<&Config>,
    ) -> Result<Vec<HashMap<String, serde_json::Value>>, String> {
        let url = format!(
            "https://stock.xueqiu.com/v5/stock/hot_stock/list.json?size={limit}&type={stock_type}"
        );
        let data = self.get_json(&url, config)?;

        let items: Vec<serde_json::Value> = data
            .get("data")
            .and_then(|d| d.get("items"))
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Ok(items
            .into_iter()
            .take(limit)
            .enumerate()
            .map(|(idx, item)| {
                let mut m = HashMap::new();
                m.insert(
                    "symbol".to_string(),
                    item.get("code")
                        .or_else(|| item.get("symbol"))
                        .cloned()
                        .unwrap_or(serde_json::Value::String(String::new())),
                );
                m.insert(
                    "name".to_string(),
                    item.get("name").cloned().unwrap_or_default(),
                );
                m.insert("current".to_string(), item.get("current").cloned().unwrap_or_default());
                m.insert("percent".to_string(), item.get("percent").cloned().unwrap_or_default());
                m.insert(
                    "rank".to_string(),
                    serde_json::Value::Number(((idx + 1) as i64).into()),
                );
                m
            })
            .collect())
    }
}

// ── Channel trait impl ───────────────────────────────────────────────

impl Channel for XueqiuChannel {
    fn name(&self) -> &str {
        "xueqiu"
    }

    fn description(&self) -> &str {
        "雪球股票行情与社区动态"
    }

    fn backends(&self) -> &[&str] {
        &["Xueqiu API (需要登录 Cookie)"]
    }

    fn tier(&self) -> u8 {
        1
    }

    fn can_handle(&self, url: &str) -> bool {
        match Url::parse(url) {
            Ok(parsed) => parsed
                .host_str()
                .map(|h| h.to_lowercase().contains("xueqiu.com"))
                .unwrap_or(false),
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

        match self.get_json(
            "https://stock.xueqiu.com/v5/stock/batch/quote.json?symbol=SH000001",
            config,
        ) {
            Ok(data) => {
                let item_count = data
                    .get("data")
                    .and_then(|d| d.get("items"))
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);

                if item_count > 0 {
                    self.active_backend =
                        Some("Xueqiu API (需要登录 Cookie)".to_string());
                    CheckResult {
                        status: CheckStatus::Ok,
                        message: "公开 API 可用（行情、搜索、热帖、热股）".to_string(),
                        active_backend: self.active_backend.clone(),
                    }
                } else {
                    CheckResult {
                        status: CheckStatus::Warn,
                        message: "API 响应异常（返回数据为空）".to_string(),
                        active_backend: None,
                    }
                }
            }
            Err(e) => CheckResult {
                status: CheckStatus::Warn,
                message: format!(
                    "Xueqiu API 连接失败：{e}。\
                     请先登录雪球后运行：agent-reach configure --from-browser chrome"
                ),
                active_backend: None,
            },
        }
    }
}

impl Default for XueqiuChannel {
    fn default() -> Self {
        Self::new()
    }
}

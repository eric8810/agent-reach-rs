//! MCP (Model Context Protocol) server — JSON-RPC 2.0 over stdio.
//!
//! Reads JSON-RPC requests line by line from stdin and writes responses
//! to stdout. Each message is a single JSON object on one line.
//!
//! Supported methods:
//! - `initialize` — returns server capabilities
//! - `tools/list` — returns list of available tools
//! - `tools/call` — calls a tool and returns result
//! - `notifications/initialized` — acknowledgment (no response needed)

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};

/// Run the MCP server loop. Blocks until stdin is closed.
pub fn run_mcp_server() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let reader = BufReader::new(stdin.lock());

    for line in reader.lines() {
        let line = match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => l,
            Err(_) => break,
        };

        let request: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let id = request.get("id").cloned();
        let method = request
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("");

        let response = match method {
            "initialize" => handle_initialize(id),
            "tools/list" => handle_tools_list(id),
            "tools/call" => handle_tools_call(id, &request),
            "notifications/initialized" => continue, // no response for notifications
            _ => make_error(id, -32601, &format!("Method not found: {}", method)),
        };

        if let Some(resp) = response {
            let mut out = stdout.lock();
            let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap_or_default());
            let _ = out.flush();
        }
    }
}

/// Build a success response. Returns `None` if there is no id (notification).
fn make_response(id: Option<Value>, result: Value) -> Option<Value> {
    let id = id?;
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Build an error response. Returns `None` if there is no id (notification).
fn make_error(id: Option<Value>, code: i64, message: &str) -> Option<Value> {
    let id = id?;
    Some(json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}))
}

/// Handle `initialize` — return server capabilities.
fn handle_initialize(id: Option<Value>) -> Option<Value> {
    make_response(
        id,
        json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {"name": "agent-reach", "version": "1.5.0"},
            "capabilities": {"tools": {}}
        }),
    )
}

/// Handle `tools/list` — return the tool catalogue.
fn handle_tools_list(id: Option<Value>) -> Option<Value> {
    make_response(
        id,
        json!({
            "tools": [{
                "name": "get_status",
                "description": "Get Agent Reach status: which 13 internet channels are installed and active.",
                "inputSchema": {"type": "object", "properties": {}}
            }]
        }),
    )
}

/// Handle `tools/call` — execute the named tool and return its result.
fn handle_tools_call(id: Option<Value>, request: &Value) -> Option<Value> {
    let tool_name = request
        .get("params")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("");

    match tool_name {
        "get_status" => {
            let config = crate::config::Config::load().unwrap_or_default();
            let results = crate::doctor::check_all(&config);
            let report = crate::doctor::format_report(&results);

            make_response(
                id,
                json!({
                    "content": [{"type": "text", "text": report}]
                }),
            )
        }
        _ => make_error(id, -32602, &format!("Unknown tool: {}", tool_name)),
    }
}

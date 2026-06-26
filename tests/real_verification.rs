//! Real HTTP verification: each channel's native backend must respond correctly.
//! These tests hit real APIs to prove the native backends actually work.

use std::process::Command;

const EXE: &str = "target/debug/agent-reach.exe";

fn doctor_json() -> serde_json::Value {
    let output = Command::new(EXE)
        .args(["doctor", "--json"])
        .output()
        .expect("failed to run doctor --json");
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).expect("doctor --json should be valid JSON")
}

/// Test that native backends are the active backends (not external CLI fallbacks)
#[test]
fn test_native_backends_active() {
    let doc = doctor_json();

    // YouTube should use youtube-native, not yt-dlp
    let yt = &doc["youtube"];
    assert_eq!(yt["active_backend"], "youtube-native", "YouTube should use native InnerTube API");
    assert_eq!(yt["status"], "ok", "YouTube native API should be ok");

    // Bilibili should use B站 API (native), not bili-cli
    let bl = &doc["bilibili"];
    assert_eq!(bl["active_backend"], "B站 API (native)", "Bilibili should use native API");
    assert_eq!(bl["status"], "ok", "Bilibili native API should be ok");

    // Xiaoyuzhou should use Whisper API (native), not groq-whisper requiring ffmpeg
    let xyz = &doc["xiaoyuzhou"];
    assert!(xyz["active_backend"].as_str().unwrap().contains("native"), "Xiaoyuzhou should use native Whisper API");

    // LinkedIn should use a native backend
    let li = &doc["linkedin"];
    let backend = li["active_backend"].as_str().unwrap_or("");
    assert!(!backend.is_empty(), "LinkedIn should have an active backend");

    // V2EX should use V2EX API (public) - always native
    let v2 = &doc["v2ex"];
    assert_eq!(v2["status"], "ok", "V2EX API should be reachable");

    // Web should be ok (Jina Reader)
    let web = &doc["web"];
    assert_eq!(web["status"], "ok", "Web channel should be ok");

    // RSS should be ok (pure Rust)
    let rss = &doc["rss"];
    assert_eq!(rss["status"], "ok", "RSS channel should be ok");
}

/// Test GitHub native API: it should probe GitHub API directly, not require gh CLI
#[test]
fn test_github_native_probe() {
    let doc = doctor_json();
    let gh = &doc["github"];

    // GitHub should probe the REST API, not just check for gh binary
    let msg = gh["message"].as_str().unwrap_or("");
    assert!(msg.contains("token") || msg.contains("Token") || msg.contains("API"),
        "GitHub should probe native API, got: {}", msg);
}

/// Test Reddit native API probe
#[test]
fn test_reddit_native_probe() {
    let doc = doctor_json();
    let rd = &doc["reddit"];

    // Reddit should know about native API
    let backends = rd["backends"].as_array().unwrap();
    let native_backend = backends.iter().any(|b| {
        b.as_str().unwrap_or("").contains("native")
    });
    assert!(native_backend, "Reddit should have native backend in backends list");
}

/// Test Twitter native API probe
#[test]
fn test_twitter_native_probe() {
    let doc = doctor_json();
    let tw = &doc["twitter"];

    let backends = tw["backends"].as_array().unwrap();
    let native_backend = backends.iter().any(|b| {
        b.as_str().unwrap_or("").contains("native")
    });
    assert!(native_backend, "Twitter should have native backend in backends list");
}

/// Test Exa native API probe
#[test]
fn test_exa_native_probe() {
    let doc = doctor_json();
    let exa = &doc["exa_search"];

    let backends = exa["backends"].as_array().unwrap();
    let native_backend = backends.iter().any(|b| {
        b.as_str().unwrap_or("").contains("native")
    });
    assert!(native_backend, "Exa should have native backend in backends list");
}

/// Test that YouTube native API is listed as a backend
#[test]
fn test_youtube_native_backend() {
    let doc = doctor_json();
    let yt = &doc["youtube"];

    let backends = yt["backends"].as_array().unwrap();
    let has_native = backends.iter().any(|b| b.as_str() == Some("youtube-native"));
    assert!(has_native, "YouTube should list youtube-native as a backend, got: {:?}", backends);
}

/// Actually call the YouTube InnerTube API to verify it works
#[test]
fn test_youtube_innertube_api_works() {
    let body = serde_json::json!({
        "videoId": "dQw4w9WgXcQ",
        "context": {
            "client": {
                "clientName": "WEB",
                "clientVersion": "2.20250623.00.00"
            }
        }
    });

    let resp = ureq::post("https://www.youtube.com/youtubei/v1/player")
        .set("Content-Type", "application/json")
        .set("X-YouTube-Client-Name", "1")
        .set("X-YouTube-Client-Version", "2.20250623.00.00")
        .query("key", "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8")
        .send_json(&body);

    match resp {
        Ok(r) => {
            let status = r.status();
            assert!(status == 200, "YouTube InnerTube API should return 200, got {}", status);
            let json: serde_json::Value = r.into_json().expect("should be valid JSON");
            let title = json["videoDetails"]["title"].as_str().unwrap_or("");
            assert!(!title.is_empty(), "Should have video title, got empty");
            println!("YouTube API works: title = {}", title);
        }
        Err(e) => {
            panic!("YouTube InnerTube API call failed: {}", e);
        }
    }
}

/// Actually call the Bilibili search API to verify it works
#[test]
fn test_bilibili_api_works() {
    let resp = ureq::get("https://api.bilibili.com/x/web-interface/search/all/v2")
        .query("keyword", "test")
        .query("page", "1")
        .set("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
        .set("Referer", "https://www.bilibili.com")
        .call();

    match resp {
        Ok(r) => {
            assert_eq!(r.status(), 200, "Bilibili search API should return 200");
            let json: serde_json::Value = r.into_json().expect("should be valid JSON");
            let code = json["code"].as_i64().unwrap_or(-1);
            assert_eq!(code, 0, "Bilibili API code should be 0, got {}", code);
            println!("Bilibili API works: code = {}", code);
        }
        Err(e) => {
            panic!("Bilibili API call failed: {}", e);
        }
    }
}

/// Actually call the V2EX API to verify it works
#[test]
fn test_v2ex_api_works() {
    let resp = ureq::get("https://www.v2ex.com/api/topics/show.json")
        .query("node_name", "python")
        .query("page", "1")
        .set("User-Agent", "agent-reach/1.0")
        .call();

    match resp {
        Ok(r) => {
            assert_eq!(r.status(), 200, "V2EX API should return 200");
            let json: serde_json::Value = r.into_json().expect("should be valid JSON");
            let arr = json.as_array().expect("should be an array");
            assert!(!arr.is_empty(), "V2EX API should return non-empty results");
            println!("V2EX API works: {} topics returned", arr.len());
        }
        Err(e) => {
            panic!("V2EX API call failed: {}", e);
        }
    }
}

/// Test Exa MCP endpoint is reachable
#[test]
fn test_exa_endpoint_reachable() {
    let resp = ureq::get("https://mcp.exa.ai/mcp").call();
    match resp {
        Ok(r) => {
            let status = r.status();
            // MCP endpoint returns 405 (Method Not Allowed) for GET, which is expected
            // The service is alive even if it returns an error code
            assert!(status == 405 || status == 200,
                "Exa MCP should be reachable (expected 405 for GET), got {}", status);
            println!("Exa MCP endpoint reachable: status {}", status);
        }
        Err(ureq::Error::Status(code, _)) => {
            // ureq treats 4xx as errors, but 405 is expected for MCP GET
            assert_eq!(code, 405, "Exa MCP expected 405, got {}", code);
            println!("Exa MCP endpoint reachable: status 405 (expected)");
        }
        Err(e) => {
            panic!("Exa MCP endpoint unreachable: {}", e);
        }
    }
}

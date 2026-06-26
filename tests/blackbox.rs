use std::process::Command;

const EXE: &str = "target/debug/agent-reach.exe";

fn run(args: &[&str]) -> (bool, String) {
    let output = Command::new(EXE)
        .args(args)
        .output()
        .expect("failed to execute agent-reach");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), format!("{}{}", stdout, stderr))
}

fn assert_ok(args: &[&str], test_name: &str) {
    let (ok, _out) = run(args);
    assert!(ok, "FAIL: {}", test_name);
}

fn assert_contains(args: &[&str], expected: &str, test_name: &str) {
    let (_ok, out) = run(args);
    assert!(
        out.contains(expected),
        "FAIL: {} — expected '{}' in output, got:\n{}",
        test_name,
        expected,
        &out[..std::cmp::min(500, out.len())]
    );
}

#[test]
fn test_cli_commands_run() {
    let commands: &[&[&str]] = &[
        &["version"],
        &["--help"],
        &["--version"],
        &["doctor"],
        &["doctor", "--json"],
        &["install", "--dry-run"],
        &["install", "--dry-run", "--channels", "twitter"],
        &["install", "--safe"],
        &["configure"],
        &["uninstall", "--dry-run"],
        &["skill", "--install"],
        &["skill", "--uninstall"],
        &["check-update"],
    ];

    for cmd in commands {
        let (ok, _) = run(cmd);
        assert!(ok, "FAIL: agent-reach {}", cmd.join(" "));
    }
}

#[test]
fn test_doctor_13_channels() {
    let (_ok, out) = run(&["doctor"]);
    assert!(out.contains("/13"), "Doctor should show /13 total");
    assert!(out.contains("V2EX"), "Doctor should mention V2EX");
    assert!(out.contains("YouTube"), "Doctor should mention YouTube");
    assert!(out.contains("Twitter"), "Doctor should mention Twitter");
    assert!(out.contains("Reddit"), "Doctor should mention Reddit");
    assert!(out.contains("RSS"), "Doctor should mention RSS");
    assert!(out.contains("GitHub"), "Doctor should mention GitHub");
    assert!(out.contains("LinkedIn"), "Doctor should mention LinkedIn");
}

fn run_stdout(args: &[&str]) -> String {
    let output = Command::new(EXE)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .expect("failed to execute agent-reach");
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_doctor_json_13_channels() {
    let out = run_stdout(&["doctor", "--json"]);
    let json: serde_json::Value = serde_json::from_str(&out).expect("doctor --json should be valid JSON");
    let obj = json.as_object().expect("should be a JSON object");
    assert_eq!(obj.len(), 13, "should have 13 channels");
    // Check a few key channels
    assert!(obj.contains_key("web"), "should have web channel");
    assert!(obj.contains_key("github"), "should have github channel");
    assert!(obj.contains_key("youtube"), "should have youtube channel");
    assert!(obj.contains_key("twitter"), "should have twitter channel");
    assert!(obj.contains_key("bilibili"), "should have bilibili channel");
    assert!(obj.contains_key("reddit"), "should have reddit channel");
    assert!(obj.contains_key("rss"), "should have rss channel");
    assert!(obj.contains_key("xiaohongshu"), "should have xiaohongshu channel");
    assert!(obj.contains_key("v2ex"), "should have v2ex channel");
    assert!(obj.contains_key("xueqiu"), "should have xueqiu channel");
    assert!(obj.contains_key("linkedin"), "should have linkedin channel");
    assert!(obj.contains_key("xiaoyuzhou"), "should have xiaoyuzhou channel");
    assert!(obj.contains_key("exa_search"), "should have exa_search channel");
}

#[test]
fn test_install_flow() {
    assert_contains(&["install", "--dry-run"], "Agent Reach Installer", "install banner");
    assert_contains(&["install", "--dry-run"], "Environment", "install env detect");
    assert_contains(&["install", "--dry-run", "--channels", "twitter"], "twitter", "install twitter channel");
    assert_contains(&["install", "--safe"], "SAFE MODE", "install safe mode");
}

#[test]
fn test_configure_flow() {
    assert_ok(&["configure", "proxy", "http://test.local:8080"], "configure proxy set");
    assert_contains(&["configure", "proxy"], "proxy =", "configure proxy get");
    assert_ok(&["configure", "groq-key", "gsk_test_abc123"], "configure groq set");
    assert_contains(&["configure", "groq-key"], "gsk_test", "configure groq get");
    assert_ok(&["configure", "openai-key", "sk-test_abc123"], "configure openai set");
    assert_contains(&["configure", "openai-key"], "sk-test", "configure openai get");
    assert_ok(&["configure", "github-token", "ghp_test123"], "configure github set");
    assert_contains(&["configure", "github-token"], "ghp_test", "configure github get");
}

#[test]
fn test_skill_install() {
    assert_ok(&["skill", "--install"], "skill install");

    let home = dirs::home_dir().expect("no home dir");
    let skill_path = home.join(".agents").join("skills").join("agent-reach").join("SKILL.md");
    assert!(skill_path.exists(), "SKILL.md should exist after install");

    let ref_dir = home.join(".agents").join("skills").join("agent-reach").join("references");
    assert!(ref_dir.exists(), "references dir should exist");
    let ref_count = std::fs::read_dir(&ref_dir)
        .map(|d| d.filter(|e| {
            e.as_ref().map(|f| f.path().extension().map_or(false, |ext| ext == "md")).unwrap_or(false)
        }).count())
        .unwrap_or(0);
    assert!(ref_count >= 4, "should have at least 4 reference files, got {}", ref_count);
}

#[test]
fn test_transcribe_error_handling() {
    let (_ok, out) = run(&["transcribe", "https://example.com/audio.mp3"]);
    // Should fail gracefully
    let has_error = out.contains("failed")
        || out.contains("not found")
        || out.contains("Transcription")
        || out.contains("yt-dlp");
    assert!(has_error, "transcribe should handle missing yt-dlp gracefully");
}

#[test]
fn test_format_xhs() {
    let input = r#"{"note_id":"999","title":"TestNote","desc":"hello world"}"#;
    let mut child = Command::new(EXE)
        .args(["format", "xhs"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn format xhs");

    use std::io::Write;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).unwrap();
    }

    let output = child.wait_with_output().expect("failed to read format output");
    let out = String::from_utf8_lossy(&output.stdout);
    assert!(out.contains("note_id"), "format output should contain note_id");
    assert!(out.contains("TestNote"), "format output should contain TestNote");
}

#[test]
fn test_mcp_server() {
    let mut child = Command::new(EXE)
        .args(["mcp-server"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn mcp-server");

    use std::io::{BufRead, BufReader, Write};
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2024-11-05","capabilities":{{}},"clientInfo":{{"name":"test","version":"1.0"}}}}}}"#).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("agent-reach"), "MCP initialize should return server info");

    // Send tools/list
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("get_status"), "MCP tools/list should include get_status");

    // Send tools/call get_status
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"get_status","arguments":{{}}}}}}"#).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("Agent Reach"), "MCP tools/call should return doctor report");

    // Send tools/call nonexistent
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"nonexistent","arguments":{{}}}}}}"#).unwrap();
    stdin.flush().unwrap();
    line.clear();
    reader.read_line(&mut line).unwrap();
    assert!(line.contains("32602") || line.contains("Unknown"), "MCP should return error for unknown tool");

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_edge_cases() {
    let (ok, out) = run(&["nonexistent"]);
    // Clap should reject unknown subcommand
    assert!(!ok || out.contains("error") || out.contains("unrecognized"), "unknown subcommand should error");

    let (_ok, out) = run(&["configure"]);
    assert!(out.contains("Usage") || out.contains("Keys"), "configure without args should show usage");
}

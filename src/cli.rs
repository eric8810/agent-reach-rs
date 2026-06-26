/// Agent Reach CLI — installer, doctor, and configuration tool.
///
/// Usage:
///     agent-reach install --env=auto
///     agent-reach doctor
///     agent-reach configure groq-key gsk_xxxxx
///     agent-reach setup

use clap::{Arg, ArgAction, Command, arg};

use crate::config::Config;
use crate::doctor;
use crate::VERSION;

/// Build the CLI argument parser.
fn build_cli() -> Command {
    Command::new("agent-reach")
        .about("Give your AI Agent eyes to see the entire internet")
        .version(concat!("Agent Reach v", env!("CARGO_PKG_VERSION")))
        .arg(arg!(-v --verbose "Show debug logs").action(ArgAction::SetTrue))
        .subcommand(Command::new("version").about("Show version"))
        .subcommand(
            Command::new("doctor")
                .about("Check platform availability")
                .arg(arg!(--json "Output machine-readable JSON instead of text report").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("install")
                .about("One-shot installer with flags")
                .arg(Arg::new("env").long("env").value_parser(["local", "server", "auto"]).default_value("auto")
                    .help("Environment: local, server, or auto-detect"))
                .arg(Arg::new("proxy").long("proxy").default_value("")
                    .help("Network proxy (http://user:pass@ip:port)"))
                .arg(arg!(--safe "Safe mode: skip automatic system changes").action(ArgAction::SetTrue))
                .arg(arg!(--"dry-run" "Show what would be done without making any changes").action(ArgAction::SetTrue))
                .arg(Arg::new("channels").long("channels").default_value("")
                    .help("Comma-separated channels (twitter,xiaoyuzhou,xueqiu,xiaohongshu,reddit,bilibili,linkedin,all)")),
        )
        .subcommand(
            Command::new("configure")
                .about("Set a config value or auto-extract from browser")
                .arg(Arg::new("key").value_parser(["proxy", "github-token", "groq-key", "openai-key", "twitter-cookies", "youtube-cookies", "xhs-cookies"])
                    .help("What to configure"))
                .arg(Arg::new("value").num_args(1..).help("The value(s) to set"))
                .arg(Arg::new("from-browser").long("from-browser")
                    .value_parser(["chrome", "firefox", "edge", "brave", "opera"])
                    .help("Auto-extract ALL platform cookies from browser")),
        )
        .subcommand(
            Command::new("uninstall")
                .about("Remove all Agent Reach config, tokens, and skill files")
                .arg(arg!(--"dry-run" "Show what would be removed").action(ArgAction::SetTrue))
                .arg(arg!(--"keep-config" "Remove skill files only").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("skill")
                .about("Manage agent skill registration")
                .arg(arg!(--install "Install SKILL.md").action(ArgAction::SetTrue))
                .arg(arg!(--uninstall "Remove SKILL.md").action(ArgAction::SetTrue)),
        )
        .subcommand(
            Command::new("transcribe")
                .about("Transcribe audio/video (Whisper via Groq/OpenAI)")
                .arg(Arg::new("source").required(true).help("Audio/video URL or local file path"))
                .arg(Arg::new("provider").long("provider").value_parser(["auto", "groq", "openai"]).default_value("auto")
                    .help("Transcription provider"))
                .arg(arg!(-o --output <FILE> "Write transcript to a file instead of stdout")),
        )
        .subcommand(Command::new("check-update").about("Check for new versions"))
        .subcommand(Command::new("watch").about("Quick health check"))
        .subcommand(Command::new("setup").about("Interactive configuration wizard"))
        .subcommand(Command::new("mcp-server").about("Start MCP server (JSON-RPC over stdio)"))
        .subcommand(
            Command::new("format")
                .about("Clean platform API output")
                .arg(arg!(<PLATFORM> "Platform to format").value_parser(["xhs"])),
        )
}

/// Run the CLI.
pub fn run() {
    let matches = build_cli().get_matches();

    let verbose = matches.get_flag("verbose");
    if verbose {
        env_logger::Builder::new().filter_level(log::LevelFilter::Info).init();
    }

    match matches.subcommand() {
        Some(("version", _)) => println!("Agent Reach v{}", VERSION),
        Some(("doctor", m)) => cmd_doctor(m.get_flag("json")),
        Some(("install", m)) => cmd_install(m),
        Some(("configure", m)) => cmd_configure(m),
        Some(("uninstall", m)) => cmd_uninstall(m),
        Some(("skill", m)) => cmd_skill(m),
        Some(("transcribe", m)) => cmd_transcribe(m),
        Some(("check-update", _)) => cmd_check_update(),
        Some(("watch", _)) => cmd_watch(),
        Some(("setup", _)) => cmd_setup(),
        Some(("mcp-server", _)) => crate::mcp_server::run_mcp_server(),
        Some(("format", m)) => cmd_format(m),
        Some((name, _)) => eprintln!("Unknown command: {}", name),
        None => {}
    }
}

// ── Command handlers ────────────────────────────────

fn cmd_doctor(json_output: bool) {
    let config = Config::load().unwrap_or_else(|_| {
        eprintln!("Warning: Could not load config, using defaults.");
        Config::default()
    });
    let results = doctor::check_all(&config);
    if json_output {
        if let Ok(json) = serde_json::to_string_pretty(&results) {
            println!("{}", json);
        }
    } else {
        println!("{}", doctor::format_report(&results));
    }
}

fn cmd_install(sub_m: &clap::ArgMatches) {
    let env = sub_m.get_one::<String>("env").map(|s| s.as_str()).unwrap_or("auto");
    let proxy = sub_m.get_one::<String>("proxy").map(|s| s.as_str()).unwrap_or("");
    let safe = sub_m.get_flag("safe");
    let dry_run = sub_m.get_flag("dry-run");
    let channels = sub_m.get_one::<String>("channels").map(|s| s.as_str()).unwrap_or("");

    crate::install::run_install(env, proxy, channels, safe, dry_run);
}

fn cmd_configure(sub_m: &clap::ArgMatches) {
    // ── Auto-extract from browser ──
    if let Some(browser) = sub_m.get_one::<String>("from-browser") {
        println!("Extracting cookies from {}...", browser);
        println!();
        let mut config = Config::load().unwrap_or_default();
        let results = crate::cookie_extract::configure_from_browser(browser, &mut config);
        let mut found = false;
        for (platform, success, message) in &results {
            if *success {
                println!("  ✅ {}: {}", platform, message);
                found = true;
            } else {
                println!("  -- {}: {}", platform, message);
            }
        }
        println!();
        if found {
            println!("✅ Cookies configured! Run `agent-reach doctor` to see updated status.");
        } else {
            eprintln!("No cookies found. Make sure you're logged into the platforms in {}.", browser);
        }
        return;
    }

    // ── Manual configure ──
    let key = sub_m.get_one::<String>("key");
    let values: Vec<String> = sub_m.get_many::<String>("value")
        .map(|vs| vs.cloned().collect())
        .unwrap_or_default();

    match (key, values.is_empty()) {
        (Some(k), false) => {
            let value = values.join(" ");
            let mut config = Config::load().unwrap_or_default();

            let config_key = key_to_config_key(k);
            match config.set(config_key, &value) {
                Ok(()) => {
                    println!("✅ {} has been configured.", k);
                    // Handle twitter-cookies special case: parse and verify
                    if k == "twitter-cookies" {
                        handle_twitter_cookies(&value);
                    }
                    // Handle xhs-cookies: try Docker injection first, fall back to config
                    if k == "xhs-cookies" {
                        handle_xhs_cookies(&value, &config);
                    }
                }
                Err(e) => eprintln!("Error saving config: {}", e),
            }
        }
        (Some(k), true) => {
            let config = Config::load().unwrap_or_default();
            let config_key = key_to_config_key(k);
            match config.get(config_key) {
                Some(v) => {
                    let display = if v.len() > 8 { format!("{}...", &v[..8]) } else { "***".to_string() };
                    println!("{} = {}", k, display);
                }
                None => println!("{} is not configured.", k),
            }
        }
        (None, _) => {
            println!("Usage: agent-reach configure <key> [value]");
            println!("   or: agent-reach configure --from-browser chrome");
            println!("Keys: proxy, github-token, groq-key, openai-key, twitter-cookies, youtube-cookies, xhs-cookies");
        }
    }
}

fn key_to_config_key(key: &str) -> &str {
    match key {
        "proxy" => "proxy",
        "github-token" => "github_token",
        "groq-key" => "groq_api_key",
        "openai-key" => "openai_api_key",
        "twitter-cookies" => "twitter_auth_token",
        "youtube-cookies" => "youtube_cookies_from",
        "xhs-cookies" => "xhs_cookies",
        _ => key,
    }
}

fn handle_twitter_cookies(value: &str) {
    // Parse twitter cookie input: either "auth_token ct0" or "auth_token=xxx; ct0=yyy"
    let (auth_token, ct0) = parse_twitter_input(value);
    if let (Some(auth), Some(ct0_val)) = (auth_token, ct0) {
        let mut config = Config::load().unwrap_or_default();
        let _ = config.set("twitter_auth_token", &auth);
        let _ = config.set("twitter_ct0", &ct0_val);
        println!("✅ Twitter cookies configured!");

        // Test access if twitter CLI is installed
        if which::which("twitter").is_ok() {
            print!("Testing Twitter access... ");
            match std::process::Command::new("twitter")
                .args(["status"])
                .env("TWITTER_AUTH_TOKEN", &auth)
                .env("TWITTER_CT0", &ct0_val)
                .output()
            {
                Ok(out) => {
                    let output = String::from_utf8_lossy(&out.stdout).to_string()
                        + &String::from_utf8_lossy(&out.stderr);
                    if output.contains("ok: true") {
                        println!("✅ Twitter access works!");
                    } else {
                        println!("[!] Auth check failed (cookies might be wrong)");
                    }
                }
                Err(_) => println!("[X] Could not test Twitter access"),
            }
        }
    } else {
        println!("[X] Could not find auth_token and ct0 in your input.");
        println!("   Accepted formats:");
        println!("   1. agent-reach configure twitter-cookies AUTH_TOKEN CT0");
        println!("   2. agent-reach configure twitter-cookies \"auth_token=xxx; ct0=yyy; ...\"");
    }
}

fn parse_twitter_input(value: &str) -> (Option<String>, Option<String>) {
    if value.contains("auth_token=") && value.contains("ct0=") {
        let mut auth = None;
        let mut ct = None;
        for part in value.replace(';', " ").split_whitespace() {
            if let Some(val) = part.strip_prefix("auth_token=") {
                auth = Some(val.to_string());
            } else if let Some(val) = part.strip_prefix("ct0=") {
                ct = Some(val.to_string());
            }
        }
        (auth, ct)
    } else {
        let parts: Vec<&str> = value.split_whitespace().collect();
        if parts.len() == 2 && !value.contains('=') {
            (Some(parts[0].to_string()), Some(parts[1].to_string()))
        } else {
            (None, None)
        }
    }
}

fn handle_xhs_cookies(value: &str, _config: &Config) {
    use std::io::Write;

    println!();
    println!("Configuring XiaoHongShu cookies...");

    // Try to find the xiaohongshu-mcp Docker container
    match std::process::Command::new("docker")
        .args(["ps", "--filter", "name=xiaohongshu-mcp", "--format", "{{.Names}}"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let container = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !container.is_empty() {
                println!("  Found Docker container: {}", container);

                let cookie_json = parse_xhs_cookie_input(value);
                let tmp = std::env::temp_dir().join(format!("xhs_cookies_{}.json", std::process::id()));
                if let Ok(mut f) = std::fs::File::create(&tmp) {
                    let _ = f.write_all(cookie_json.as_bytes());
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
                    }
                }

                let cp_result = std::process::Command::new("docker")
                    .args(["cp", &tmp.to_string_lossy(), &format!("{}:/app/cookies.json", container)])
                    .output();
                let _ = std::fs::remove_file(&tmp);

                match cp_result {
                    Ok(o) if o.status.success() => {
                        println!("  Cookies injected into container {}", container);

                        match std::process::Command::new("docker")
                            .args(["restart", &container])
                            .output()
                        {
                            Ok(o) if o.status.success() => {
                                println!("  Container restarted, waiting for service...");
                                std::thread::sleep(std::time::Duration::from_secs(3));
                                if let Ok(v) = std::process::Command::new("mcporter")
                                    .args(["call", "xiaohongshu.check_login_status()", "--timeout", "30000"])
                                    .output()
                                {
                                    let out = String::from_utf8_lossy(&v.stdout);
                                    println!("  {}", out.trim());
                                }
                            }
                            Err(e) => eprintln!("  Container restart failed: {}", e),
                            Ok(_) => eprintln!("  Container restart failed"),
                        }
                    }
                    Err(e) => eprintln!("  docker cp failed: {}", e),
                    Ok(_) => eprintln!("  docker cp failed"),
                }
                return;
            }
            println!("  No xiaohongshu-mcp container found.");
        }
        Ok(_) => println!("  Docker not available."),
        Err(_) => println!("  Docker check skipped."),
    }

    println!("  Saving cookies to config file.");
}

fn parse_xhs_cookie_input(value: &str) -> String {
    if value.trim().starts_with('{') || value.trim().starts_with('[') {
        if serde_json::from_str::<serde_json::Value>(value).is_ok() {
            return value.to_string();
        }
    }
    if value.contains('=') && !value.trim().starts_with('{') {
        let mut cookie_map = serde_json::Map::new();
        for pair in value.split(';') {
            let pair = pair.trim();
            if let Some((k, v)) = pair.split_once('=') {
                cookie_map.insert(
                    k.trim().to_string(),
                    serde_json::Value::String(v.trim().to_string()),
                );
            }
        }
        if !cookie_map.is_empty() {
            let mut root = serde_json::Map::new();
            root.insert("cookies".to_string(), serde_json::Value::Object(cookie_map));
            if let Ok(json) = serde_json::to_string(&root) {
                return json;
            }
        }
    }
    value.to_string()
}

fn cmd_uninstall(sub_m: &clap::ArgMatches) {
    let dry_run = sub_m.get_flag("dry-run");
    let keep_config = sub_m.get_flag("keep-config");

    if dry_run {
        println!("DRY RUN — showing what would be removed:");
    }

    let config_dir = Config::config_dir();
    if config_dir.exists() {
        if keep_config {
            println!("Keeping config at: {}", config_dir.display());
        } else if dry_run {
            println!("[dry-run] Would remove: {}", config_dir.display());
        } else {
            match std::fs::remove_dir_all(&config_dir) {
                Ok(()) => println!("✅ Removed config directory: {}", config_dir.display()),
                Err(e) => eprintln!("Warning: Could not remove config: {}", e),
            }
        }
    } else {
        println!("No config directory found at: {}", config_dir.display());
    }

    // Also uninstall skill
    if !keep_config {
        let _ = crate::skill::uninstall_skill();
    }

    // Clean up mcporter MCP entries (best-effort)
    if !dry_run && !keep_config {
        for entry in &["exa", "xiaohongshu"] {
            if let Ok(output) = std::process::Command::new("mcporter")
                .args(["config", "remove", entry])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
            {
                if output.status.success() {
                    println!("  ✅ Removed mcporter entry: {}", entry);
                }
            }
        }
    }

    println!("✅ Uninstall complete.");
}

fn cmd_skill(sub_m: &clap::ArgMatches) {
    if sub_m.get_flag("install") {
        match crate::skill::install_skill() {
            Ok(()) => {}
            Err(e) => eprintln!("Skill installation failed: {}", e),
        }
    } else if sub_m.get_flag("uninstall") {
        match crate::skill::uninstall_skill() {
            Ok(()) => {}
            Err(e) => eprintln!("Skill uninstallation failed: {}", e),
        }
    }
}

fn cmd_transcribe(sub_m: &clap::ArgMatches) {
    let source = sub_m.get_one::<String>("source").map(|s| s.as_str()).unwrap_or("");
    let provider = sub_m.get_one::<String>("provider").map(|s| s.as_str()).unwrap_or("auto");
    let output_file = sub_m.get_one::<String>("output");

    println!("⏳ Transcribing: {}", source);
    println!("   Provider: {}", provider);

    let config = Config::load().ok();

    match crate::transcribe::transcribe(source, provider, config.as_ref()) {
        Ok(text) => {
            if let Some(path) = output_file {
                match std::fs::write(path, &text) {
                    Ok(()) => println!("✅ Transcript written to {}", path),
                    Err(e) => eprintln!("Error writing to {}: {}", path, e),
                }
            } else {
                println!("{}", text);
            }
        }
        Err(e) => {
            eprintln!("❌ Transcription failed: {}", e);
        }
    }
}

fn cmd_check_update() {
    let current = VERSION.trim_start_matches('v');
    println!("Agent Reach v{}", VERSION);
    println!("Checking for updates...");

    let url = "https://api.github.com/repos/eric8810/agent-reach-rs/releases/latest";
    match ureq::get(url)
        .set("User-Agent", "agent-reach")
        .set("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(10))
        .call()
    {
        Ok(resp) => {
            if let Ok(json) = resp.into_json::<serde_json::Value>() {
                if let Some(tag) = json.get("tag_name").and_then(|v| v.as_str()) {
                    let latest = tag.trim_start_matches('v');
                    if latest != current {
                        println!();
                        println!("🔔 New version available: v{} (current: v{})", latest, current);
                        if let Some(body) = json.get("body").and_then(|v| v.as_str()) {
                            let preview: String = body.lines().take(15).collect::<Vec<_>>().join("\n");
                            println!();
                            println!("Release notes:");
                            println!("{}", preview);
                        }
                        if let Some(html_url) = json.get("html_url").and_then(|v| v.as_str()) {
                            println!();
                            println!("Download: {}", html_url);
                        }
                    } else {
                        println!("✅ You are running the latest version.");
                    }
                    return;
                }
            }
            eprintln!("Warning: Could not parse GitHub release info.");
        }
        Err(ureq::Error::Status(403, _)) => {
            eprintln!("Warning: GitHub API rate limit reached. Try again later.");
        }
        Err(e) => {
            eprintln!("Warning: Could not check for updates: {}", e);
        }
    }
    println!("Check https://github.com/eric8810/agent-reach-rs/releases for updates.");
}

fn cmd_watch() {
    let config = Config::load().unwrap_or_default();
    let results = doctor::check_all(&config);
    let ok_count = results.values().filter(|r| r.status == "ok").count();
    let total = results.len();
    println!("Agent Reach v{} — {}/{} channels active", VERSION, ok_count, total);

    // Report broken/warning channels
    let mut issues: Vec<String> = Vec::new();
    for (_name, r) in &results {
        if r.status == "error" {
            issues.push(format!("  [X] {} — {}", r.name, r.message.lines().next().unwrap_or("unknown error")));
        } else if r.status == "off" {
            issues.push(format!("  -- {} — {}", r.name, r.message.lines().next().unwrap_or("not installed")));
        }
    }
    if !issues.is_empty() {
        println!();
        println!("Channels needing attention:");
        for issue in &issues {
            println!("{}", issue);
        }
    }

    // Also check for updates (best-effort)
    cmd_check_update();
}

fn cmd_setup() {
    use std::io::{self, Write};

    println!("Interactive configuration wizard");
    println!("{}", "≡".repeat(40));
    println!();
    println!("Agent Reach configuration wizard helps you set up:");
    println!("  1. Exa search (via mcporter, free)");
    println!("  2. GitHub token (for private repos, rate limits)");
    println!("  3. Reddit setup guide");
    println!("  4. Groq API key (for audio transcription, free)");
    println!();

    let mut config = Config::load().unwrap_or_default();
    let stdin = io::stdin();
    let mut input = String::new();

    // Step 1: mcporter + Exa
    println!("── Step 1/4: Exa search ──");
    if crate::probe::command_exists("mcporter") {
        // Check if Exa is already configured
        if let Ok(output) = std::process::Command::new("mcporter")
            .args(["config", "list"])
            .output()
        {
            let out = String::from_utf8_lossy(&output.stdout);
            if out.contains("exa") {
                println!("✅ Exa search is already configured.");
            } else {
                print!("Exa search not configured. Configure now? [Y/n]: ");
                io::stdout().flush().ok();
                input.clear();
                stdin.read_line(&mut input).ok();
                if !input.trim().eq_ignore_ascii_case("n") {
                    match std::process::Command::new("mcporter")
                        .args(["config", "add", "exa", "https://mcp.exa.ai/mcp"])
                        .output()
                    {
                        Ok(o) if o.status.success() => println!("✅ Exa search configured!"),
                        _ => println!("  Could not configure Exa. Run: mcporter config add exa https://mcp.exa.ai/mcp"),
                    }
                }
            }
        }
    } else {
        println!("  mcporter not installed. Install with: npm install -g mcporter");
        println!("  Then: mcporter config add exa https://mcp.exa.ai/mcp");
    }

    // Step 2: GitHub token
    println!();
    println!("── Step 2/4: GitHub token ──");
    if config.get("github_token").map_or(false, |v| !v.is_empty()) {
        println!("✅ GitHub token is already configured.");
    } else {
        println!("  A GitHub token unlocks private repos and higher rate limits.");
        println!("  Get one at: https://github.com/settings/tokens (no special scopes needed)");
        print!("  Enter GitHub token (or press Enter to skip): ");
        io::stdout().flush().ok();
        input.clear();
        stdin.read_line(&mut input).ok();
        let token = input.trim();
        if !token.is_empty() {
            config.set("github_token", token).ok();
            println!("✅ GitHub token saved!");
        }
    }

    // Step 3: Reddit
    println!();
    println!("── Step 3/4: Reddit ──");
    if crate::probe::command_exists("rdt") {
        println!("✅ rdt-cli detected. Run `rdt login` if not yet authenticated.");
    } else {
        println!("  Reddit requires rdt-cli (pipx install rdt-cli) and login cookie.");
        println!("  See: https://raw.githubusercontent.com/Panniantong/agent-reach/main/agent_reach/guides/setup-reddit.md");
    }

    // Step 4: Groq API key
    println!();
    println!("── Step 4/4: Groq API key ──");
    let has_groq = std::env::var("GROQ_API_KEY").map_or(false, |v| !v.is_empty())
        || config.get("groq_api_key").map_or(false, |v| !v.is_empty());
    if has_groq {
        println!("✅ Groq API key is configured.");
    } else {
        println!("  Groq provides free Whisper API access for audio transcription.");
        println!("  Sign up at: https://console.groq.com");
        print!("  Enter Groq API key (or press Enter to skip): ");
        io::stdout().flush().ok();
        input.clear();
        stdin.read_line(&mut input).ok();
        let key = input.trim();
        if !key.is_empty() {
            config.set("groq_api_key", key).ok();
            println!("✅ Groq API key saved!");
        }
    }

    println!();
    println!("{}", "≡".repeat(40));
    println!("✅ Setup complete! Config saved to: {}", Config::config_file().display());
    println!("Run `agent-reach doctor` to check all channels.");
}

fn cmd_format(sub_m: &clap::ArgMatches) {
    let platform = sub_m.get_one::<String>("PLATFORM").map(|s| s.as_str()).unwrap_or("xhs");

    match platform {
        "xhs" => {
            // Read from stdin
            let mut input = String::new();
            match std::io::Read::read_to_string(&mut std::io::stdin(), &mut input) {
                Ok(_) => {
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        eprintln!("Error: no input on stdin");
                        return;
                    }
                    match serde_json::from_str::<serde_json::Value>(trimmed) {
                        Ok(data) => {
                            let cleaned = crate::channels::xiaohongshu::format_xhs_result(&data);
                            if let Ok(json) = serde_json::to_string_pretty(&cleaned) {
                                println!("{}", json);
                            }
                        }
                        Err(e) => eprintln!("Error: invalid JSON: {}", e),
                    }
                }
                Err(e) => eprintln!("Error reading stdin: {}", e),
            }
        }
        _ => eprintln!("Unknown platform: {}", platform),
    }
}

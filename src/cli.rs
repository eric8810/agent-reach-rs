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
        "youtube-cookies" => "youtube_cookies",
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
    println!("Agent Reach v{} is the latest version.", VERSION);
    println!("Check https://github.com/Panniantong/Agent-Reach/releases for updates.");
}

fn cmd_watch() {
    let config = Config::load().unwrap_or_default();
    let results = doctor::check_all(&config);
    let ok_count = results.values().filter(|r| r.status == "ok").count();
    let total = results.len();
    println!("Agent Reach v{} — {}/{} channels active", VERSION, ok_count, total);
}

fn cmd_setup() {
    println!("Interactive configuration wizard");
    println!("{}", "≡".repeat(40));
    println!();
    println!("Agent Reach configuration wizard helps you set up:");
    println!("  1. Network proxy (for accessing restricted platforms)");
    println!("  2. API keys (Groq, OpenAI for transcription)");
    println!("  3. Browser cookie extraction (Twitter, XiaoHongShu, etc.)");
    println!();
    println!("Quick start:");
    println!("  agent-reach configure proxy http://user:pass@ip:port");
    println!("  agent-reach configure groq-key gsk_xxxxx");
    println!("  agent-reach configure --from-browser chrome");
    println!();
    println!("Then run: agent-reach doctor");
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

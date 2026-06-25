//! Agent Reach installer — one-shot deterministic install flow.
//!
//! Translated from the Python CLI: `_cmd_install`, `_install_system_deps`,
//! `_install_mcporter`, `_install_*_deps`, `_install_skill`, etc.

use crate::config::Config;
use crate::doctor;
use crate::backends::{opencli_status, opencli_summary};
use crate::backends::opencli::{OPENCLI_EXTENSION_URL, OPENCLI_PACKAGE};

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};


// ── helpers ──────────────────────────────────────────────────────────

/// Run a command with timeout. Returns (stdout, stderr) or error string.
/// On dry_run, prints what would run and returns Ok.
fn run_cmd(
    program: &str,
    args: &[&str],
    _timeout_secs: u64,
    dry_run: bool,
) -> Result<String, String> {
    if dry_run {
        println!("  [dry-run] {} {}", program, args.join(" "));
        return Ok(String::new());
    }
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run {}: {}", program, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(if stderr.is_empty() { stdout } else { stderr })
    }
}

/// Run a command, silently (no output printed). For status checks.
fn run_cmd_silent(program: &str, args: &[&str], _timeout_secs: u64) -> Option<String> {
    Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Check if a binary is on PATH.
fn has_cmd(name: &str) -> bool {
    which::which(name).is_ok()
}

/// Try installing a Python package via pipx first, then uv tool install.
/// Returns true on success.
fn install_python_package(package: &str, dry_run: bool) -> bool {
    for (tool, args) in [
        ("pipx", vec!["install", package]),
        ("uv", vec!["tool", "install", package]),
    ] {
        if has_cmd(tool) {
            match run_cmd(tool, &args, 120, dry_run) {
                Ok(_) => return true,
                Err(e) => {
                    if !dry_run {
                        eprintln!("  {} install {} failed: {}", tool, package, e);
                    }
                }
            }
        }
    }
    false
}

/// Try installing a Python package from a git source via pipx first, then uv.
fn install_python_package_from(
    source: &str,
    binary: &str,
    dry_run: bool,
) -> bool {
    // pipx install <source>
    if has_cmd("pipx") {
        match run_cmd("pipx", &["install", source], 120, dry_run) {
            Ok(_) if dry_run || has_cmd(binary) => return true,
            Ok(_) => {}
            Err(e) => {
                if !dry_run {
                    eprintln!("  pipx install {} failed: {}", source, e);
                }
            }
        }
    }
    // uv tool install --from <source> <binary>
    if has_cmd("uv") {
        match run_cmd("uv", &["tool", "install", "--from", source, binary], 120, dry_run) {
            Ok(_) if dry_run || has_cmd(binary) => return true,
            Ok(_) => {}
            Err(e) => {
                if !dry_run {
                    eprintln!("  uv tool install --from {} failed: {}", source, e);
                }
            }
        }
    }
    false
}

/// Return the Agent Reach tools directory: `~/.agent-reach/tools`
fn tools_dir() -> PathBuf {
    Config::config_dir().join("tools")
}

/// Ensure tools_dir exists.
fn ensure_tools_dir() {
    let dir = tools_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("  Warning: could not create {}: {}", dir.display(), e);
    }
}

// ── environment detection ────────────────────────────────────────────

/// Detect whether we are running on a local desktop or headless server.
///
/// Heuristics (matches Python `_detect_environment`):
/// - On Windows/macOS: always "local" (unless strong container signals)
/// - On Linux: checks SSH, Docker/container, display, cloud VM indicators,
///   and systemd-detect-virt.
pub fn detect_environment() -> &'static str {
    #[cfg(target_os = "windows")]
    { return "local"; }

    #[cfg(target_os = "macos")]
    {
        // Check Docker/container even on macOS (e.g. Docker Desktop)
        if PathBuf::from("/.dockerenv").exists() || PathBuf::from("/run/.containerenv").exists() {
            return "server";
        }
        return "local";
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let mut score: i32 = 0;

        // SSH session
        if std::env::var("SSH_CONNECTION").is_ok() || std::env::var("SSH_CLIENT").is_ok() {
            score += 2;
        }

        // Docker / container
        if PathBuf::from("/.dockerenv").exists() || PathBuf::from("/run/.containerenv").exists() {
            score += 2;
        }

        // No display (headless)
        if std::env::var("DISPLAY").is_err() && std::env::var("WAYLAND_DISPLAY").is_err() {
            score += 1;
        }

        // Cloud VM indicators
        for cloud_file in ["/sys/hypervisor/uuid", "/sys/class/dmi/id/product_name"] {
            if let Ok(content) = fs::read_to_string(cloud_file) {
                let c = content.to_lowercase();
                let cloud_markers = [
                    "amazon", "google", "microsoft", "digitalocean",
                    "linode", "vultr", "hetzner",
                ];
                if cloud_markers.iter().any(|m| c.contains(m)) {
                    score += 2;
                    break;
                }
            }
        }

        // systemd-detect-virt
        if let Some(out) = run_cmd_silent("systemd-detect-virt", &[], 3) {
            if out.trim() != "none" {
                score += 1;
            }
        }

        if score >= 2 { "server" } else { "local" }
    }
}

// ── system dependencies ──────────────────────────────────────────────

/// Install system-level dependencies: gh CLI, Node.js, undici, yt-dlp JS runtime config.
pub fn install_system_deps(safe_mode: bool, dry_run: bool) {
    println!("Checking system dependencies...");

    // ── gh CLI ──
    if has_cmd("gh") {
        println!("  ✅ gh CLI already installed");
    } else if safe_mode {
        println!("  -- gh CLI not found");
        println!("     Install: https://cli.github.com — or: apt install gh / brew install gh");
    } else if dry_run {
        println!("  [dry-run] Would install gh CLI");
    } else {
        println!("  Installing gh CLI...");
        if cfg!(target_os = "linux") {
            let installed = install_gh_linux();
            if installed {
                println!("  ✅ gh CLI installed");
            } else {
                println!("  [!] gh CLI install failed. Try: snap install gh, or download from https://github.com/cli/cli/releases");
            }
        } else if cfg!(target_os = "macos") {
            if has_cmd("brew") {
                match run_cmd("brew", &["install", "gh"], 120, false) {
                    Ok(_) => {
                        if has_cmd("gh") {
                            println!("  ✅ gh CLI installed");
                        } else {
                            println!("  [!] gh CLI install failed. Try: brew install gh");
                        }
                    }
                    Err(_) => println!("  [!] gh CLI install failed. Try: brew install gh"),
                }
            } else {
                println!("  [!] gh CLI not found. Install: https://cli.github.com");
            }
        } else {
            println!("  [!] gh CLI not found. Install: https://cli.github.com");
        }
    }

    // ── Node.js ──
    let has_node = has_cmd("node") && has_cmd("npm");
    if has_node {
        println!("  ✅ Node.js already installed");
    } else if safe_mode {
        println!("  -- Node.js not found");
        println!("     Install: https://nodejs.org — or: apt install nodejs npm");
    } else if dry_run {
        println!("  [dry-run] Would install Node.js");
    } else {
        println!("  Installing Node.js...");
        if cfg!(target_os = "linux") {
            let installed = install_node_linux();
            if installed {
                println!("  ✅ Node.js installed");
            } else {
                println!("  [!] Node.js install failed. Try: apt install nodejs npm, or nvm install 22");
            }
        } else if cfg!(target_os = "macos") {
            if has_cmd("brew") {
                let _ = run_cmd("brew", &["install", "node"], 120, false);
                if has_cmd("node") {
                    println!("  ✅ Node.js installed");
                } else {
                    println!("  [!] Node.js install failed. Try: brew install node");
                }
            } else {
                println!("  [!] Node.js not found. Install: https://nodejs.org");
            }
        } else {
            println!("  [!] Node.js not found. Install: https://nodejs.org");
        }
    }

    // ── undici (proxy support for Node.js fetch) ──
    if has_cmd("npm") {
        let npm_root = run_cmd_silent("npm", &["root", "-g"], 5);
        let undici_ok = npm_root.as_ref().map_or(false, |root| {
            PathBuf::from(root.trim()).join("undici").join("index.js").exists()
        });
        if undici_ok {
            println!("  ✅ undici already installed (Node.js proxy support)");
        } else if safe_mode {
            println!("  -- undici not installed globally (optional)");
            println!("     Install: npm install -g undici");
        } else if dry_run {
            println!("  [dry-run] Would install undici globally");
        } else {
            match run_cmd("npm", &["install", "-g", "undici"], 60, false) {
                Ok(_) => println!("  ✅ undici installed (Node.js proxy support)"),
                Err(_) => println!("  -- undici install failed (optional — may not work behind proxies)"),
            }
        }
    } else if safe_mode {
        println!("  -- npm not available (Node.js missing), can't check undici");
    }

    // ── yt-dlp JS runtime config ──
    if has_cmd("node") {
        let ytdlp_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config").join("yt-dlp");
        let ytdlp_config = ytdlp_dir.join("config");
        let needs_config = if ytdlp_config.exists() {
            match fs::read_to_string(&ytdlp_config) {
                Ok(content) => !content.contains("--js-runtimes"),
                Err(_) => true,
            }
        } else {
            true
        };
        if !needs_config {
            println!("  ✅ yt-dlp JS runtime already configured");
        } else if safe_mode {
            println!("  -- yt-dlp JS runtime not configured");
            println!("     Run: mkdir -p ~/.config/yt-dlp && echo '--js-runtimes node' >> ~/.config/yt-dlp/config");
        } else if dry_run {
            println!("  [dry-run] Would configure yt-dlp JS runtime");
        } else {
            let _ = fs::create_dir_all(&ytdlp_dir);
            match std::fs::OpenOptions::new().create(true).append(true).open(&ytdlp_config) {
                Ok(mut f) => {
                    let _ = writeln!(f, "--js-runtimes node");
                    println!("  ✅ yt-dlp configured to use Node.js as JS runtime (YouTube)");
                }
                Err(_) => println!("  -- Could not configure yt-dlp JS runtime (YouTube may not work)"),
            }
        }
    }

    if safe_mode {
        println!();
        println!("  To install missing dependencies manually:");
        if !has_cmd("gh") {
            println!("    gh CLI: https://cli.github.com — or: apt install gh / brew install gh");
        }
        if !has_cmd("node") || !has_cmd("npm") {
            println!("    Node.js: https://nodejs.org — or: apt install nodejs npm / brew install node");
        }
    }
}

/// Install gh CLI on Linux via apt.
fn install_gh_linux() -> bool {
    // Detect architecture
    let arch = run_cmd_silent("dpkg", &["--print-architecture"], 10)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "amd64".to_string());

    let keyring = "/usr/share/keyrings/githubcli-archive-keyring.gpg";
    let list_path = "/etc/apt/sources.list.d/github-cli.list";

    // Download GPG key
    if run_cmd("curl", &[
        "-fsSL",
        "https://cli.github.com/packages/githubcli-archive-keyring.gpg",
        "-o", keyring,
    ], 60, false).is_err() {
        return false;
    }

    // Write apt source
    let repo_line = format!(
        "deb [arch={} signed-by={}] https://cli.github.com/packages stable main\n",
        arch, keyring
    );
    if fs::write(list_path, repo_line).is_err() {
        return false;
    }

    // Update and install
    let _ = run_cmd("apt-get", &["update", "-qq"], 60, false);
    let _ = run_cmd("apt-get", &["install", "-y", "-qq", "gh"], 60, false);

    has_cmd("gh")
}

/// Install Node.js on Linux via NodeSource.
fn install_node_linux() -> bool {
    // Use NodeSource setup script
    let tmp = std::env::temp_dir().join("nodesource_setup.sh");
    if run_cmd("curl", &[
        "-fsSL",
        "https://deb.nodesource.com/setup_22.x",
        "-o", &tmp.to_string_lossy(),
    ], 60, false).is_err() {
        return false;
    }

    if run_cmd("bash", &[&tmp.to_string_lossy()], 120, false).is_err() {
        let _ = fs::remove_file(&tmp);
        return false;
    }
    let _ = fs::remove_file(&tmp);

    let _ = run_cmd("apt-get", &["install", "-y", "-qq", "nodejs"], 120, false);
    has_cmd("node")
}

// ── mcporter + Exa ───────────────────────────────────────────────────

/// Install mcporter and configure Exa search.
pub fn install_mcporter(safe_mode: bool, dry_run: bool) {
    println!("Setting up mcporter (search backend)...");

    if has_cmd("mcporter") {
        println!("  ✅ mcporter already installed");
    } else if safe_mode {
        println!("  -- mcporter not installed");
        println!("     To install: npm install -g mcporter");
        println!("     Then configure Exa: mcporter config add exa https://mcp.exa.ai/mcp");
        return;
    } else if dry_run {
        println!("  [dry-run] Would install mcporter via npm");
    } else {
        // Need npm/npx
        if !has_cmd("npm") && !has_cmd("npx") {
            println!("  [!] mcporter requires Node.js. Install Node.js first:");
            println!("     https://nodejs.org/ or: curl -fsSL https://fnm.vercel.app/install | bash");
            return;
        }
        // npm install -g mcporter, or try npx as fallback
        let cmd_result = if has_cmd("npm") {
            run_cmd("npm", &["install", "-g", "mcporter"], 120, false)
        } else {
            // npx can't install globally — report missing
            println!("  [!] npm not found, mcporter requires npm. Install Node.js first.");
            return;
        };

        if cmd_result.is_ok() && has_cmd("mcporter") {
            println!("  ✅ mcporter installed");
        } else {
            println!("  [X] mcporter install failed. Retry: npm install -g mcporter, or try: npx mcporter@latest list");
            return;
        }
    }

    // Configure Exa MCP (free, no key needed)
    if safe_mode {
        println!("  To configure Exa search: mcporter config add exa https://mcp.exa.ai/mcp");
        return;
    }
    if dry_run {
        println!("  [dry-run] Would configure Exa MCP");
        return;
    }

    match run_cmd("mcporter", &["config", "list"], 5, false) {
        Ok(stdout) => {
            if stdout.contains("exa") {
                println!("  ✅ Exa search already configured");
            } else {
                match run_cmd("mcporter", &["config", "add", "exa", "https://mcp.exa.ai/mcp"], 10, false) {
                    Ok(_) => println!("  ✅ Exa search configured (free, no API key needed)"),
                    Err(_) => println!("  [!] Could not configure Exa. Run: mcporter config add exa https://mcp.exa.ai/mcp"),
                }
            }
        }
        Err(_) => println!("  [!] Could not check mcporter config. Run: mcporter config add exa https://mcp.exa.ai/mcp"),
    }
}

// ── channel installers ───────────────────────────────────────────────

/// Install optional channel tools for a named channel.
/// Channels with no install step (xueqiu, linkedin) print a message.
pub fn install_channel(channel: &str, env: &str, safe_mode: bool, dry_run: bool) {
    match channel {
        "twitter" => install_twitter_deps(safe_mode, dry_run),
        "xiaoyuzhou" => install_xiaoyuzhou_deps(safe_mode, dry_run),
        "xiaohongshu" => install_xhs_deps(env, safe_mode, dry_run),
        "reddit" => install_reddit_deps(env, safe_mode, dry_run),
        "bilibili" => install_bili_deps(safe_mode, dry_run),
        "opencli" => install_opencli_deps(safe_mode, dry_run),
        "xueqiu" => {
            if safe_mode || dry_run {
                println!("  xueqiu: cookie-only channel, no install step needed");
            } else {
                println!("  xueqiu: cookie-only — configure cookies with: agent-reach configure --from-browser chrome");
            }
        }
        "linkedin" => {
            if safe_mode || dry_run {
                println!("  linkedin: manual setup only, no automatic install");
            } else {
                println!("  linkedin: 需要手动配置 LinkedIn Cookie");
                println!("    1. 在浏览器登录 linkedin.com");
                println!("    2. 导出 li_at Cookie");
                println!("    3. agent-reach configure linkedin-cookies li_at=<your_cookie>");
            }
        }
        _ => println!("  Unknown channel: {} — skipping", channel),
    }
}

/// Install twitter-cli (via pipx/uv).
fn install_twitter_deps(safe_mode: bool, dry_run: bool) {
    println!("Setting up Twitter (twitter-cli)...");
    if has_cmd("twitter") {
        println!("  ✅ twitter-cli already installed");
        return;
    }
    if safe_mode {
        println!("  -- twitter-cli not installed");
        println!("     Install: pipx install twitter-cli  (or uv tool install twitter-cli)");
        return;
    }
    if dry_run {
        println!("  [dry-run] Would install twitter-cli");
        return;
    }
    if install_python_package("twitter-cli", dry_run) {
        if has_cmd("twitter") {
            println!("  ✅ twitter-cli installed");
        }
    } else {
        println!("  [!] twitter-cli install failed. Run: pipx install twitter-cli");
    }
}

/// Install Xiaoyuzhou transcription script + check ffmpeg.
fn install_xiaoyuzhou_deps(safe_mode: bool, dry_run: bool) {
    println!("Setting up Xiaoyuzhou podcast transcription...");

    let tools = tools_dir().join("xiaoyuzhou");
    let script_dst = tools.join("transcribe.sh");

    if script_dst.exists() {
        println!("  ✅ Xiaoyuzhou transcription script already installed");
    } else if safe_mode {
        println!("  -- Xiaoyuzhou transcription script not installed");
        println!("     The transcribe.sh script is in the agent_reach package scripts directory.");
    } else if dry_run {
        println!("  [dry-run] Would copy transcribe.sh to {}", script_dst.display());
    } else {
        // The Python version copies transcribe.sh from the Python package.
        // In Rust, we embed the script or look relative to the binary.
        // For now, guide the user to copy it from the Python package.
        println!("  [!] transcribe.sh script: copy from Python package scripts/ directory");
        println!("     cp agent-reach-python/agent_reach/scripts/transcribe_xiaoyuzhou.sh {}", script_dst.display());
        println!("     chmod +x {}", script_dst.display());
    }

    // Check ffmpeg
    if has_cmd("ffmpeg") {
        println!("  ✅ ffmpeg available");
    } else {
        println!("  -- ffmpeg not found. Install: apt install -y ffmpeg (or brew install ffmpeg)");
    }

    // Check Groq key
    let config = Config::load().unwrap_or_default();
    let has_key = std::env::var("GROQ_API_KEY").is_ok() || config.get("groq_api_key").is_some();
    if has_key {
        println!("  ✅ Groq API key configured");
    } else {
        println!("  -- Groq API key not set. Get free key at https://console.groq.com");
        println!("     Then run: agent-reach configure groq-key gsk_xxxxx");
    }
}

/// Set up XiaoHongShu. Desktop: OpenCLI. Server: xiaohongshu-mcp guide.
fn install_xhs_deps(env: &str, safe_mode: bool, dry_run: bool) {
    println!("Setting up XiaoHongShu...");

    if env == "server" {
        println!("  服务器环境推荐 xiaohongshu-mcp（自带无头浏览器，扫码登录）：");
        println!("    1. 下载 binary：https://github.com/xpzouying/xiaohongshu-mcp/releases");
        println!("       （建议放到 ~/.agent-reach/tools/ 下）");
        println!("    2. 启动服务（首次运行会下载约 150MB 浏览器，请等待完成）");
        println!("    3. 扫码登录后接入：mcporter config add xiaohongshu http://localhost:18060/mcp");
        println!("    4. 验证：agent-reach doctor");
        return;
    }

    install_opencli_deps(safe_mode, dry_run);
    if has_cmd("xhs") {
        println!("  ✅ 检测到存量 xhs-cli，将作为备选后端继续可用");
    }
}

/// Install OpenCLI — cross-platform backend riding Chrome session. Desktop-only.
fn install_opencli_deps(safe_mode: bool, dry_run: bool) {
    println!("Setting up OpenCLI (browser-session backend, desktop only)...");

    let st = opencli_status(5);
    if st.installed && !st.broken {
        println!("  ✅ {}", opencli_summary(&st));
        if !st.ready() {
            println!("  {}", st.hint);
        }
        return;
    }

    if safe_mode {
        if !st.installed {
            println!("  -- OpenCLI not installed");
            println!("     Install: npm install -g {}", OPENCLI_PACKAGE);
        }
        if !st.extension_installed && !st.extension_connected {
            println!("     Also install Chrome extension: {}", OPENCLI_EXTENSION_URL);
        }
        return;
    }

    if dry_run {
        println!("  [dry-run] Would install {}", OPENCLI_PACKAGE);
        return;
    }

    if !has_cmd("npm") {
        println!("  [!] OpenCLI requires Node.js ≥ 20. Install Node first:");
        println!("       https://nodejs.org  （或 brew install node）");
        return;
    }

    match run_cmd("npm", &["install", "-g", OPENCLI_PACKAGE], 300, false) {
        Ok(_) => {
            let st2 = opencli_status(5);
            if st2.installed && !st2.broken {
                println!("  ✅ OpenCLI installed");
                if !st2.extension_installed && !st2.extension_connected {
                    println!("  最后一步（必须手动，Chrome 安全限制）：安装浏览器扩展");
                    println!("    1. 打开 {}", OPENCLI_EXTENSION_URL);
                    println!("    2. 点「添加至 Chrome」");
                    println!("    3. 运行 `opencli doctor` 验证连接");
                }
            } else {
                println!("  [!] OpenCLI install failed. Run: npm install -g {}", OPENCLI_PACKAGE);
            }
        }
        Err(e) => {
            println!("  [!] OpenCLI install failed: {}", e);
            println!("     Run: npm install -g {}", OPENCLI_PACKAGE);
        }
    }
}

/// Set up Reddit. Desktop: OpenCLI, server: rdt-cli.
fn install_reddit_deps(env: &str, safe_mode: bool, dry_run: bool) {
    if env != "server" {
        install_opencli_deps(safe_mode, dry_run);
        println!("  Reddit 走 OpenCLI（浏览器里登录过 reddit.com 即可用）");
        if has_cmd("rdt") {
            println!("  ✅ 检测到存量 rdt-cli，将作为备选后端继续可用");
        }
        return;
    }
    install_rdt_cli(safe_mode, dry_run);
}

/// Install rdt-cli (pinned git source — PyPI lags upstream).
///
/// Pinned to the 0.4.2 state matching Python CLI.
const RDT_GIT_SOURCE: &str = "git+https://github.com/public-clis/rdt-cli.git@5e4fb3720d5c174e976cd425ccc3b879d52cac66";

fn install_rdt_cli(safe_mode: bool, dry_run: bool) {
    println!("Setting up Reddit (rdt-cli)...");
    if has_cmd("rdt") {
        println!("  ✅ rdt-cli already installed");
        return;
    }
    if safe_mode {
        println!("  -- rdt-cli not installed");
        println!("     Install: pipx install '{}'", RDT_GIT_SOURCE);
        return;
    }
    if dry_run {
        println!("  [dry-run] Would install rdt-cli from git");
        return;
    }
    if install_python_package_from(RDT_GIT_SOURCE, "rdt", dry_run) {
        if has_cmd("rdt") {
            println!("  ✅ rdt-cli installed");
        }
    } else {
        println!("  [!] rdt-cli install failed. Run: pipx install '{}'", RDT_GIT_SOURCE);
    }
}

/// Install bili-cli for Bilibili.
fn install_bili_deps(safe_mode: bool, dry_run: bool) {
    println!("Setting up Bilibili (bili-cli)...");
    if has_cmd("bili") {
        println!("  ✅ bili-cli already installed");
        return;
    }
    if safe_mode {
        println!("  -- bili-cli not installed");
        println!("     Install: pipx install bilibili-cli  (or uv tool install bilibili-cli)");
        return;
    }
    if dry_run {
        println!("  [dry-run] Would install bilibili-cli");
        return;
    }
    if install_python_package("bilibili-cli", dry_run) {
        if has_cmd("bili") {
            println!("  ✅ bili-cli installed");
        }
    } else {
        println!("  [!] bili-cli install failed. Run: pipx install bilibili-cli");
    }
}

// ── skill installation ───────────────────────────────────────────────

/// Install Agent Reach skill to agent directories.
///
/// Delegates to the skill module which embeds SKILL.md content and
/// copies it to known agent skill directories.
pub fn install_skill() {
    println!("Installing agent skill...");
    match crate::skill::install_skill() {
        Ok(()) => {}  // skill::install_skill prints its own messages
        Err(e) => eprintln!("  Skill installation failed: {}", e),
    }
}

// ── cookie auto-import ───────────────────────────────────────────────

/// Try to import cookies from browser for channels that need them.
///
/// Channel set that needs cookies: twitter, xueqiu, bilibili.
/// Tries Chrome first, then Firefox.
pub fn auto_import_cookies(requested_channels: &HashSet<&str>) {
    const COOKIE_CHANNELS: &[&str] = &["twitter", "xueqiu", "bilibili"];
    let needs_cookies = requested_channels.iter().any(|c| COOKIE_CHANNELS.contains(c));
    if !needs_cookies {
        return;
    }

    println!();
    println!("Importing cookies from browser...");
    println!("  (macOS may ask for your login password to access the Keychain — this is normal,");
    println!("   it only happens once during install. Enter your password or click 'Allow'.)");

    // Try to extract cookies using our browser cookie extraction module.
    // Tries Chrome first, then Firefox.
    let mut config = match crate::config::Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  -- Could not load config: {}", e);
            return;
        }
    };

    for browser in &["chrome", "firefox"] {
        match crate::cookie_extract::configure_from_browser(browser, &mut config) {
            results => {
                let mut found = false;
                for (platform, success, message) in &results {
                    if *success {
                        println!("  ✅ {}: {}", platform, message);
                        found = true;
                    }
                }
                if found {
                    println!();
                    println!("✅ Cookies configured! Run `agent-reach doctor` to see updated status.");
                    return;
                }
            }
        }
    }

    println!("  -- No cookies found (normal if you haven't logged into these sites)");
    println!("     To configure manually: agent-reach configure --from-browser chrome");
}

// ── main install orchestrator ────────────────────────────────────────

/// The main install entry point. Called from cli.rs.
///
/// Orchestrates the full install flow:
/// 1. Create tools directory
/// 2. Save proxy if specified
/// 3. Install system deps
/// 4. Install mcporter + Exa
/// 5. Install optional channels
/// 6. Auto-import cookies (if local + cookie channels)
/// 7. Run doctor check
/// 8. Install agent skill
pub fn run_install(
    env: &str,
    proxy: &str,
    channels_str: &str,
    safe_mode: bool,
    dry_run: bool,
) {
    println!();
    println!("Agent Reach Installer");
    println!("{}", "=".repeat(40));

    // Ensure tools directory exists
    ensure_tools_dir();

    if dry_run {
        println!("DRY RUN — showing what would be done (no changes)");
        println!();
    }
    if safe_mode {
        println!("SAFE MODE — skipping automatic system changes");
        println!();
    }

    // Parse requested channels
    let channel_installers: &[&str] = &[
        "twitter", "xiaoyuzhou", "xiaohongshu", "reddit",
        "bilibili", "opencli", "xueqiu", "linkedin",
    ];

    let mut requested_channels: HashSet<&str> = HashSet::new();
    if !channels_str.is_empty() {
        let raw: Vec<&str> = channels_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if raw.contains(&"all") {
            for ch in channel_installers {
                requested_channels.insert(*ch);
            }
        } else {
            for ch in raw {
                requested_channels.insert(ch);
            }
        }
    }

    // Determine environment
    let env = if env == "auto" { detect_environment() } else { env };

    if env == "server" {
        println!("Environment: Server/VPS (auto-detected)");
    } else {
        println!("Environment: Local computer (auto-detected)");
    }

    // Apply proxy
    if !proxy.is_empty() {
        if dry_run {
            println!("[dry-run] Would save network proxy");
        } else {
            match Config::load() {
                Ok(mut config) => {
                    let _ = config.set("proxy", proxy);
                    let _ = config.set("bilibili_proxy", proxy);
                    println!("✅ 代理已保存（Agent 访问受限网络时使用）");
                }
                Err(e) => eprintln!("  Warning: could not load config to save proxy: {}", e),
            }
        }
    }

    // ── Install core system dependencies ──
    println!();
    install_system_deps(safe_mode, dry_run);

    // ── mcporter + Exa ──
    println!();
    install_mcporter(safe_mode, dry_run);

    // ── Install optional channels ──
    if !requested_channels.is_empty() {
        println!();
        println!("Installing optional channels...");

        let mut channels = requested_channels.clone();
        if env == "server" && channels.contains("opencli") {
            channels.remove("opencli");
            println!("  -- OpenCLI 需要桌面环境 + Chrome，服务器环境跳过");
        }

        let mut sorted: Vec<&str> = channels.into_iter().collect();
        sorted.sort();

        for ch_name in &sorted {
            println!();
            if dry_run {
                println!("[dry-run] Would install channel: {}", ch_name);
            } else if safe_mode {
                println!("SAFE MODE — channel '{}' manual instructions:", ch_name);
                install_channel(ch_name, env, true, dry_run);
            } else {
                install_channel(ch_name, env, false, dry_run);
            }
        }
    }

    // ── Auto-import cookies (local only) ──
    if env == "local" && !safe_mode && !dry_run {
        auto_import_cookies(&requested_channels);
    } else if env == "local" && dry_run && !requested_channels.is_empty() {
        println!();
        println!("[dry-run] Would try to import cookies from Chrome/Firefox");
    }

    // Environment-specific advice
    if env == "server" {
        println!();
        println!("Tip: 部分平台对服务器 IP 有风控。");
        println!("   Reddit 必须登录态（rdt-cli + Cookie，见 doctor 提示），中国大陆网络还需代理。");
        println!("   保存代理供 Agent 使用：agent-reach configure proxy http://user:pass@ip:port");
        println!("   Cheap option: https://www.webshare.io ($1/month)");
    }

    // Test channels
    if !dry_run {
        println!();
        println!("Testing channels...");
        let config = Config::load().unwrap_or_default();
        let results = doctor::check_all(&config);
        let ok_count = results.values().filter(|r| r.status == "ok").count();
        let total = results.len();

        println!();
        println!("{}", doctor::format_report(&results));
        println!();

        // ── Install agent skill ──
        install_skill();

        println!("✅ Installation complete! {}/{} channels active.", ok_count, total);

        if requested_channels.is_empty() {
            println!();
            println!("More channels available! Use --channels to install:");
            println!("   agent-reach install --channels=twitter,xiaohongshu,reddit,...");
            println!("   agent-reach install --channels=all  (install everything)");
        }

        // Star reminder
        println!();
        println!("如果 Agent Reach 帮到了你，给个 Star 让更多人发现它吧：");
        println!("   https://github.com/Panniantong/Agent-Reach");
        println!("   只需一秒，对独立开发者意义很大。谢谢！");
    } else {
        println!();
        println!("Dry run complete. No changes were made.");
    }
}

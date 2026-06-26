//! Lightweight upstream command probing.
//!
//! Distinguishes the three failure modes that look identical to which:
//! - missing: command not on PATH
//! - broken: command exists but cannot execute — most commonly a stale venv
//!   shebang after a system Python upgrade
//! - timeout/error: command runs but misbehaves
//!
//! Channels use probe_command() inside check() so doctor reports real health,
//! not just file existence.

/// Result of probing an external command.
#[derive(Debug, Clone)]
pub struct ProbeResult {
    pub status: ProbeStatus,
    pub output: String,
    pub hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeStatus {
    /// Command ran successfully.
    Ok,
    /// Command not found on PATH.
    Missing,
    /// Command exists but cannot execute (stale venv / broken install).
    Broken,
    /// Command timed out.
    Timeout,
    /// Command ran but returned an error.
    Error,
}

impl ProbeResult {
    pub fn ok(&self) -> bool {
        self.status == ProbeStatus::Ok
    }
}

impl ProbeStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProbeStatus::Ok => "ok",
            ProbeStatus::Missing => "missing",
            ProbeStatus::Broken => "broken",
            ProbeStatus::Timeout => "timeout",
            ProbeStatus::Error => "error",
        }
    }
}

/// Shell exit codes for "found but not executable" / "not found".
const BROKEN_EXIT_CODES: [i32; 2] = [126, 127];

/// Generate a reinstall hint for a broken package install.
pub fn reinstall_hint(package: &str) -> String {
    format!(
        "命令存在但无法执行——通常是系统 Python 升级后 venv 解释器丢失。重装即可修复：\n  uv tool install --force {}\n或：pipx reinstall {}",
        package, package
    )
}

/// Generate a reinstall hint for npm packages.
pub fn npm_reinstall_hint(package: &str) -> String {
    format!(
        "命令存在但无法执行（node 环境损坏），重装：\n  npm install -g {}",
        package
    )
}

/// Actually execute `cmd *args` and classify the result.
///
/// Intended for SIDE-EFFECT-FREE health probes only (version/status commands).
/// `package`: pip/pipx package name used in the broken-install hint (defaults to cmd).
/// `hint_fn`: function to generate reinstall hint (default: reinstall_hint).
pub fn probe_command(
    cmd: &str,
    args: &[&str],
    timeout_secs: u64,
    retries: usize,
    package: Option<&str>,
) -> ProbeResult {
    probe_command_with_hint(cmd, args, timeout_secs, retries, package, None)
}

/// Like probe_command but with a custom hint function.
pub fn probe_command_with_hint(
    cmd: &str,
    args: &[&str],
    timeout_secs: u64,
    retries: usize,
    package: Option<&str>,
    hint_fn: Option<fn(&str) -> String>,
) -> ProbeResult {
    let hint_gen = hint_fn.unwrap_or(reinstall_hint);

    // Check if command exists on PATH
    let path = match which::which(cmd) {
        Ok(p) => p,
        Err(_) => {
            return ProbeResult {
                status: ProbeStatus::Missing,
                output: String::new(),
                hint: String::new(),
            };
        }
    };

    let package = package.unwrap_or(cmd);
    let mut last: Option<ProbeResult> = None;

    for _ in 0..=retries {
        let result = run_once(&path, args, timeout_secs, package, hint_gen);
        if result.ok() {
            return result;
        }
        // missing/broken won't heal between retries
        if matches!(result.status, ProbeStatus::Missing | ProbeStatus::Broken) {
            return result;
        }
        last = Some(result);
    }

    last.unwrap_or_else(|| ProbeResult {
        status: ProbeStatus::Error,
        output: String::new(),
        hint: "Failed after retries".to_string(),
    })
}

fn run_once(
    path: &std::path::Path,
    args: &[&str],
    timeout_secs: u64,
    package: &str,
    hint_fn: fn(&str) -> String,
) -> ProbeResult {
    let child = match std::process::Command::new(path)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            // which() found it but exec failed: the shebang interpreter is gone
            if e.kind() == std::io::ErrorKind::NotFound {
                return ProbeResult {
                    status: ProbeStatus::Broken,
                    output: String::new(),
                    hint: hint_fn(package),
                };
            }
            return ProbeResult {
                status: ProbeStatus::Broken,
                output: e.to_string(),
                hint: hint_fn(package),
            };
        }
    };

    // Wait with timeout. Capture PID before moving child into thread.
    let child_pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    let output = match rx.recv_timeout(std::time::Duration::from_secs(timeout_secs)) {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            return ProbeResult {
                status: ProbeStatus::Broken,
                output: e.to_string(),
                hint: hint_fn(package),
            };
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // Kill the timed-out child process
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &child_pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &child_pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            return ProbeResult {
                status: ProbeStatus::Timeout,
                output: format!("`{}` timed out after {}s", path.display(), timeout_secs),
                hint: hint_fn(package),
            };
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            return ProbeResult {
                status: ProbeStatus::Broken,
                output: "child process disappeared".to_string(),
                hint: hint_fn(package),
            };
        }
    };

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .trim()
    .to_string();

    if let Some(code) = output.status.code() {
        if BROKEN_EXIT_CODES.contains(&code) {
            return ProbeResult {
                status: ProbeStatus::Broken,
                output: combined,
                hint: hint_fn(package),
            };
        }
        if code != 0 {
            return ProbeResult {
                status: ProbeStatus::Error,
                output: combined,
                hint: String::new(),
            };
        }
    }

    ProbeResult {
        status: ProbeStatus::Ok,
        output: combined,
        hint: String::new(),
    }
}

/// Simple check: does the command exist on PATH?
pub fn command_exists(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

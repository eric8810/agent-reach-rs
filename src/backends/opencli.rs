//! OpenCLI backend probing.
//!
//! OpenCLI (github.com/jackwener/opencli) drives the user's real Chrome via a
//! browser-bridge extension + local daemon, reusing existing login sessions —
//! zero per-platform configuration, desktop-only (no headless).

use crate::probe::probe_command;

pub const OPENCLI_PACKAGE: &str = "@jackwener/opencli";
pub const OPENCLI_EXTENSION_ID: &str = "ildkmabpimmkaediidaifkhjpohdnifk";
pub const OPENCLI_EXTENSION_URL: &str = "https://chromewebstore.google.com/detail/opencli/ildkmabpimmkaediidaifkhjpohdnifk";

/// Chrome-family profile roots that contain <Profile>/Extensions/<id>/.
const CHROME_PROFILE_ROOTS: &[&str] = &[
    "~/Library/Application Support/Google/Chrome", // macOS Chrome
    "~/Library/Application Support/Chromium",      // macOS Chromium
    "~/.config/google-chrome",                     // Linux Chrome
    "~/.config/chromium",                          // Linux Chromium
];

#[derive(Debug, Clone)]
pub struct OpenCLIStatus {
    pub installed: bool,
    pub broken: bool,
    pub daemon_running: bool,
    pub extension_connected: bool,
    pub extension_installed: bool,
    pub version: String,
    pub hint: String,
}

impl OpenCLIStatus {
    /// Usable now or on first call.
    pub fn ready(&self) -> bool {
        self.installed && !self.broken && (self.extension_connected || self.extension_installed)
    }
}

impl Default for OpenCLIStatus {
    fn default() -> Self {
        OpenCLIStatus {
            installed: false,
            broken: false,
            daemon_running: false,
            extension_connected: false,
            extension_installed: false,
            version: String::new(),
            hint: String::new(),
        }
    }
}

/// Check if the OpenCLI extension exists in any Chrome profile.
fn extension_installed_on_disk() -> bool {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let home_str = home.to_string_lossy();

    let mut roots: Vec<String> = CHROME_PROFILE_ROOTS
        .iter()
        .map(|p| {
            if p.starts_with("~/") {
                p.replacen('~', &home_str, 1)
            } else {
                p.to_string()
            }
        })
        .collect();

    // Windows
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        roots.push(format!(
            "{}/Google/Chrome/User Data",
            local_app_data
        ));
    }

    for root in &roots {
        // Check for Extension directory in profile subdirectories
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let ext_path = entry.path().join("Extensions").join(OPENCLI_EXTENSION_ID);
                if ext_path.exists() {
                    return true;
                }
            }
        }
    }
    false
}

/// Probe OpenCLI install + daemon/extension state without side effects.
pub fn opencli_status(timeout: u64) -> OpenCLIStatus {
    let version_probe = probe_command(
        "opencli",
        &["--version"],
        timeout,
        0,
        Some(OPENCLI_PACKAGE),
    );

    if version_probe.status == crate::probe::ProbeStatus::Missing {
        return OpenCLIStatus::default();
    }

    if !version_probe.ok() {
        return OpenCLIStatus {
            installed: true,
            broken: true,
            hint: format!(
                "opencli 命令存在但无法执行（node 环境损坏），重装：\n  npm install -g {}",
                OPENCLI_PACKAGE
            ),
            ..Default::default()
        };
    }

    let mut st = OpenCLIStatus {
        installed: true,
        version: version_probe.output.trim().to_string(),
        ..Default::default()
    };

    let daemon_probe = probe_command(
        "opencli",
        &["daemon", "status"],
        timeout,
        0,
        Some(OPENCLI_PACKAGE),
    );

    let output = if daemon_probe.ok() {
        daemon_probe.output
    } else {
        String::new()
    };

    for line in output.lines() {
        let line = line.trim().to_lowercase();
        if line.starts_with("daemon:") {
            st.daemon_running = !line.contains("not running") && line.contains("running");
        } else if line.starts_with("extension:") {
            st.extension_connected = !line.contains("disconnected") && line.contains("connected");
        }
    }

    if !st.extension_connected {
        st.extension_installed = extension_installed_on_disk();
        if !st.extension_installed {
            st.hint = format!(
                "OpenCLI 已安装，但 Chrome 扩展未安装。\n  1. 安装扩展（需手动点一次）：{}\n  2. 保持 Chrome 打开，运行 `opencli doctor` 验证",
                OPENCLI_EXTENSION_URL
            );
        }
    }

    st
}

/// One-line state description for channel messages / install output.
pub fn opencli_summary(st: &OpenCLIStatus) -> String {
    if !st.installed {
        return "OpenCLI 未安装".to_string();
    }
    if st.broken {
        return "OpenCLI 无法执行（node 环境损坏）".to_string();
    }
    if st.extension_connected {
        return format!("OpenCLI 可用（浏览器登录态，v{}）", st.version);
    }
    if st.ready() {
        return "OpenCLI 可用（扩展睡眠中，调用时自动唤醒）".to_string();
    }
    if st.daemon_running {
        return "OpenCLI 已安装，等待 Chrome 扩展安装".to_string();
    }
    "OpenCLI 已安装（daemon 未运行，使用时自动启动；需 Chrome 扩展）".to_string()
}

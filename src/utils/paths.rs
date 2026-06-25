//! Cross-platform path and remediation helpers for yt-dlp.

use std::path::PathBuf;

/// Return the recommended yt-dlp user config directory for this OS.
pub fn get_ytdlp_config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        if !appdata.is_empty() {
            return PathBuf::from(appdata).join("yt-dlp");
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("AppData").join("Roaming").join("yt-dlp")
    }
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library").join("Application Support").join("yt-dlp")
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config").join("yt-dlp")
    }
}

/// Return the yt-dlp user config file path for this OS.
pub fn get_ytdlp_config_path() -> PathBuf {
    get_ytdlp_config_dir().join("config")
}

/// Return an OS-appropriate command to enable Node.js as yt-dlp JS runtime.
pub fn render_ytdlp_fix_command() -> String {
    let config_path = get_ytdlp_config_path();

    #[cfg(target_os = "windows")]
    {
        let cfg = config_path.to_string_lossy();
        format!(
            "$cfg = '{}'\n\
             New-Item -ItemType Directory -Force -Path (Split-Path $cfg) | Out-Null\n\
             if (-not (Test-Path $cfg) -or -not (Select-String -Path $cfg -Pattern '--js-runtimes' -Quiet)) {{\n\
               Add-Content -Path $cfg -Value '--js-runtimes node'\n\
             }}",
            cfg
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        let cfg_dir = config_path.parent().unwrap().to_string_lossy();
        let cfg = config_path.to_string_lossy();
        format!(
            "mkdir -p '{}' && grep -qxF -- '--js-runtimes node' '{}' 2>/dev/null || printf '%s\\n' '--js-runtimes node' >> '{}'",
            cfg_dir, cfg, cfg
        )
    }
}

/// Tools directory for Agent Reach: `~/.agent-reach/tools`
pub fn tools_dir() -> PathBuf {
    crate::config::Config::config_dir().join("tools")
}

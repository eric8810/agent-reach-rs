//! ffmpeg auto-downloader.
//!
//! If ffmpeg is not found on PATH, downloads a pre-built static binary
//! to ~/.agent-reach/tools/ffmpeg and uses it from there.
//!
//! Sources (pre-built static binaries, no compilation needed):
//!   Windows: BtbN/FFmpeg-Builds (GitHub releases)
//!   macOS:   evermeet.cx (static builds)
//!   Linux:   johnvansickle.com (static amd64 builds)

use std::path::PathBuf;

/// URL templates for static ffmpeg binary downloads.
#[cfg(target_os = "windows")]
const FFMPEG_URL: &str =
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip";

#[cfg(target_os = "macos")]
const FFMPEG_URL: &str = "https://evermeet.cx/ffmpeg/ffmpeg-7.0.zip";

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const FFMPEG_URL: &str =
    "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz";

/// Where we store downloaded tools.
fn tools_dir() -> PathBuf {
    crate::config::Config::config_dir().join("tools")
}

/// Path to our managed ffmpeg binary.
pub fn ffmpeg_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        tools_dir().join("ffmpeg").join("bin").join("ffmpeg.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        tools_dir().join("ffmpeg").join("ffmpeg")
    }
}

/// Find ffmpeg: system PATH first, then our managed copy.
pub fn find_ffmpeg() -> Option<PathBuf> {
    // 1. System PATH
    if let Ok(path) = which::which("ffmpeg") {
        return Some(path);
    }
    // 2. Our managed copy
    let managed = ffmpeg_path();
    if managed.exists() {
        return Some(managed);
    }
    None
}

/// Download and extract ffmpeg if not present.
pub fn ensure_ffmpeg(dry_run: bool) -> Result<PathBuf, String> {
    if let Some(path) = find_ffmpeg() {
        return Ok(path);
    }
    download_ffmpeg(dry_run)
}

/// Download and extract ffmpeg static build.
fn download_ffmpeg(dry_run: bool) -> Result<PathBuf, String> {
    let dest = ffmpeg_path();

    if dry_run {
        println!("  [dry-run] Would download ffmpeg from {}", FFMPEG_URL);
        println!("  [dry-run] Would extract to {}", dest.display());
        return Err("dry run".into());
    }

    println!("  Downloading ffmpeg (~30MB)...");

    // Download to temp file
    let tmp = std::env::temp_dir().join(format!("ffmpeg_dl_{}", std::process::id()));
    let resp = ureq::get(FFMPEG_URL)
        .call()
        .map_err(|e| format!("Failed to download ffmpeg: {}", e))?;

    let mut file = std::fs::File::create(&tmp)
        .map_err(|e| format!("Cannot create temp file: {}", e))?;
    std::io::copy(&mut resp.into_reader(), &mut file)
        .map_err(|e| format!("Download failed: {}", e))?;

    // Extract
    let extract_dir = tools_dir().join("ffmpeg");
    let _ = std::fs::create_dir_all(&extract_dir);

    let ext = if cfg!(target_os = "windows") { "zip" } else if cfg!(target_os = "macos") { "zip" } else { "tar.xz" };

    println!("  Extracting ffmpeg...");
    extract_archive(&tmp, &extract_dir, ext)?;

    // Find the actual ffmpeg binary inside
    let ffmpeg_bin = locate_extracted_ffmpeg(&extract_dir)?;

    // Clean up temp
    let _ = std::fs::remove_file(&tmp);

    Ok(ffmpeg_bin)
}

fn extract_archive(archive: &std::path::Path, dest: &std::path::Path, ext: &str) -> Result<(), String> {
    match ext {
        "zip" => {
            // Use PowerShell on Windows, unzip on others
            #[cfg(target_os = "windows")]
            {
                let status = std::process::Command::new("powershell")
                    .args(["-Command",
                        &format!("Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                            archive.display(), dest.display())])
                    .status()
                    .map_err(|e| format!("Failed to run unzip: {}", e))?;
                if !status.success() {
                    return Err("unzip failed".into());
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let status = std::process::Command::new("unzip")
                    .args(["-o", &archive.to_string_lossy(), "-d", &dest.to_string_lossy()])
                    .status()
                    .map_err(|e| format!("Failed to run unzip: {}", e))?;
                if !status.success() {
                    return Err("unzip failed".into());
                }
            }
        }
        "tar.xz" => {
            let status = std::process::Command::new("tar")
                .args(["-xf", &archive.to_string_lossy(), "-C", &dest.to_string_lossy()])
                .status()
                .map_err(|e| format!("Failed to run tar: {}", e))?;
            if !status.success() {
                return Err("tar extract failed".into());
            }
        }
        _ => return Err(format!("Unknown archive format: {}", ext)),
    }
    Ok(())
}

fn locate_extracted_ffmpeg(extract_dir: &std::path::Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        // BtbN builds extract to ffmpeg-master-latest-win64-gpl/bin/ffmpeg.exe
        for entry in std::fs::read_dir(extract_dir).map_err(|e| format!("read_dir: {}", e))? {
            let entry = entry.map_err(|e| format!("entry: {}", e))?;
            let candidate = entry.path().join("bin").join("ffmpeg.exe");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let candidate = extract_dir.join("ffmpeg");
        if candidate.exists() {
            return Ok(candidate);
        }
        // Look one level deep (extracted folder)
        for entry in std::fs::read_dir(extract_dir).map_err(|e| format!("read_dir: {}", e))? {
            let entry = entry.map_err(|e| format!("entry: {}", e))?;
            let candidate = entry.path().join("ffmpeg");
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    Err("Could not find ffmpeg binary in extracted archive".into())
}

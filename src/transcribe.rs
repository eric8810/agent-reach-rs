//! Whisper audio transcription with Groq → OpenAI fallback.
//!
//! Downloads audio (yt-dlp), compresses + chunks (ffmpeg), and posts to a
//! Whisper-compatible API. Defaults to Groq's free `whisper-large-v3` and falls
//! back to OpenAI's `whisper-1` on HTTP error.
//!
//! Public entry point:
//!     transcribe(source, provider, config) -> Result<String, String>

use crate::config::Config;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

// Whisper API limit is 25MB; leave headroom for multipart overhead.
const SIZE_LIMIT_BYTES: u64 = 24 * 1024 * 1024;
const CHUNK_SECONDS: u64 = 600; // 10 min

struct ProviderInfo {
    endpoint: &'static str,
    model: &'static str,
    key_field: &'static str,
}

const PROVIDERS: &[(&str, ProviderInfo)] = &[
    (
        "groq",
        ProviderInfo {
            endpoint: "https://api.groq.com/openai/v1/audio/transcriptions",
            model: "whisper-large-v3",
            key_field: "groq_api_key",
        },
    ),
    (
        "openai",
        ProviderInfo {
            endpoint: "https://api.openai.com/v1/audio/transcriptions",
            model: "whisper-1",
            key_field: "openai_api_key",
        },
    ),
];

fn get_provider(name: &str) -> Result<&'static ProviderInfo, String> {
    for (n, p) in PROVIDERS {
        if *n == name {
            return Ok(p);
        }
    }
    Err(format!(
        "unknown provider: {} (use groq|openai|auto)",
        name
    ))
}

/// Resolve provider order from the user-facing name.
fn provider_order(provider: &str) -> Result<Vec<&'static str>, String> {
    match provider {
        "auto" => Ok(vec!["groq", "openai"]),
        "groq" => Ok(vec!["groq"]),
        "openai" => Ok(vec!["openai"]),
        other => Err(format!(
            "unknown provider: {} (use groq|openai|auto)",
            other
        )),
    }
}

// ---------------------------------------------------------------------------
// Shell helpers
// ---------------------------------------------------------------------------

/// Check that a required binary exists on PATH.
fn require_binary(name: &str) -> Result<(), String> {
    which::which(name).map_err(|_| format!("{} not found in PATH", name))?;
    Ok(())
}

/// Run a subprocess, capture output, enforce a timeout, return on success or
/// a descriptive error string.
fn run_command(cmd: &str, args: &[&str], timeout_secs: u64) -> Result<(), String> {
    let child = std::process::Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {}", cmd, e))?;

    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });

    match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(output)) => {
            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let preview: String = stderr.chars().take(300).collect();
                Err(format!(
                    "{} failed (exit {}): {}",
                    cmd,
                    output.status.code().unwrap_or(-1),
                    preview
                ))
            }
        }
        Ok(Err(e)) => Err(format!("{} wait error: {}", cmd, e)),
        Err(_) => {
            // Best-effort kill on timeout — cross-platform.
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("kill")
                    .args(["-9", &pid.to_string()])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
            Err(format!("{} timed out after {}s", cmd, timeout_secs))
        }
    }
}

// ---------------------------------------------------------------------------
// Audio pipeline
// ---------------------------------------------------------------------------

/// Download audio from `url` via yt-dlp, placing output into `out_dir`.
/// Returns the path of the produced file.
fn download_audio(url: &str, out_dir: &Path) -> Result<PathBuf, String> {
    require_binary("yt-dlp")?;
    let template = out_dir.join("source.%(ext)s");
    let template_str = template.to_string_lossy().to_string();

    run_command(
        "yt-dlp",
        &[
            "-x",
            "--audio-format",
            "m4a",
            "--audio-quality",
            "0",
            "-o",
            &template_str,
            url,
        ],
        1800, // generous timeout for long podcasts over slow networks
    )?;

    let mut files: Vec<PathBuf> = std::fs::read_dir(out_dir)
        .map_err(|e| format!("cannot read temp dir: {}", e))?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.starts_with("source."))
        })
        .collect();

    if files.is_empty() {
        return Err("yt-dlp produced no output file".to_string());
    }

    // Pick the first match (there is usually exactly one).
    files.sort();
    Ok(files.remove(0))
}

/// Re-encode to mono / 16 kHz / 32 kbps m4a — keeps most content under 25 MB.
fn compress_audio(src: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    require_binary("ffmpeg")?;
    let dst = out_dir.join("compressed.m4a");
    let src_str = src.to_string_lossy();
    let dst_str = dst.to_string_lossy();

    run_command(
        "ffmpeg",
        &[
            "-loglevel",
            "error",
            "-y",
            "-i",
            &src_str,
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-b:a",
            "32k",
            &dst_str,
        ],
        600,
    )?;

    Ok(dst)
}

/// Split `src` into 10‑minute segments.  Returns paths sorted by name.
fn chunk_audio(src: &Path, out_dir: &Path) -> Result<Vec<PathBuf>, String> {
    require_binary("ffmpeg")?;
    let pattern = out_dir.join("chunk_%03d.m4a");
    let src_str = src.to_string_lossy();
    let pattern_str = pattern.to_string_lossy();

    run_command(
        "ffmpeg",
        &[
            "-loglevel",
            "error",
            "-y",
            "-i",
            &src_str,
            "-f",
            "segment",
            "-segment_time",
            &CHUNK_SECONDS.to_string(),
            "-ac",
            "1",
            "-ar",
            "16000",
            "-b:a",
            "32k",
            &pattern_str,
        ],
        600,
    )?;

    let mut chunks: Vec<PathBuf> = std::fs::read_dir(out_dir)
        .map_err(|e| format!("cannot read temp dir: {}", e))?
        .filter_map(|entry| entry.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |n| n.starts_with("chunk_") && n.ends_with(".m4a"))
        })
        .collect();

    if chunks.is_empty() {
        return Err("ffmpeg produced no chunks".to_string());
    }
    chunks.sort();
    Ok(chunks)
}

// ---------------------------------------------------------------------------
// HTTP upload helpers
// ---------------------------------------------------------------------------

/// Build a multipart/form-data body containing the audio file plus model and
/// response_format fields.  Returns (body_bytes, boundary_string).
fn build_multipart_body(file_path: &Path, model: &str) -> Result<(Vec<u8>, String), String> {
    let file_data =
        std::fs::read(file_path).map_err(|e| format!("cannot read {}: {}", file_path.display(), e))?;

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio.m4a");

    let boundary = "----FormBoundary7MA4YWxk";
    let mut body = Vec::new();

    // -- file part
    body.extend_from_slice(
        format!(
            "--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: audio/m4a\r\n\r\n",
            boundary, filename
        )
        .as_bytes(),
    );
    body.extend_from_slice(&file_data);
    body.extend_from_slice(b"\r\n");

    // -- model field
    body.extend_from_slice(
        format!(
            "--{}\r\nContent-Disposition: form-data; name=\"model\"\r\n\r\n{}\r\n",
            boundary, model
        )
        .as_bytes(),
    );

    // -- response_format field
    body.extend_from_slice(
        format!(
            "--{}\r\nContent-Disposition: form-data; name=\"response_format\"\r\n\r\ntext\r\n",
            boundary
        )
        .as_bytes(),
    );

    // -- closing boundary
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());

    Ok((body, boundary.to_string()))
}

/// Extract the provider's API key from config.
fn get_provider_key(provider_name: &str, config: &Config) -> Option<String> {
    let info = get_provider(provider_name).ok()?;
    config.get(info.key_field).filter(|v| !v.is_empty())
}

/// Transcribe a single chunk via the named provider.
fn transcribe_chunk(
    chunk: &Path,
    provider_name: &str,
    config: &Config,
) -> Result<String, String> {
    let info = get_provider(provider_name)?;

    let key = get_provider_key(provider_name, config).ok_or_else(|| {
        format!(
            "{}: missing {} (configure with `agent-reach configure {}-key ...`)",
            provider_name, info.key_field, provider_name
        )
    })?;

    let (body, boundary) = build_multipart_body(chunk, info.model)?;

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(30))
        .timeout_read(Duration::from_secs(120))
        .build();

    let resp = agent
        .post(info.endpoint)
        .set("Authorization", &format!("Bearer {}", key))
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={}", boundary),
        )
        .send_bytes(&body)
        .map_err(|e| {
            // Distinguish transport errors from HTTP-level errors.
            match &e {
                ureq::Error::Status(code, _) => {
                    format!("{}: HTTP {}", provider_name, code)
                }
                ureq::Error::Transport(_) => {
                    format!("{}: network error: {}", provider_name, e)
                }
            }
        })?;

    resp.into_string().map_err(|e| {
        format!(
            "{}: failed to read response body: {}",
            provider_name, e
        )
    })
}

/// Try each provider in order for a single chunk; return first success or the
/// last error.
fn transcribe_chunk_with_fallback(
    chunk: &Path,
    order: &[&str],
    config: &Config,
) -> Result<String, String> {
    let mut last_err: Option<String> = None;
    for &provider_name in order {
        // Silently skip providers without a configured key — caller already
        // validated that at least one provider has a key.
        if get_provider_key(provider_name, config).is_none() {
            continue;
        }
        match transcribe_chunk(chunk, provider_name, config) {
            Ok(text) => return Ok(text),
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        }
    }
    let chunk_name = chunk
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    Err(format!(
        "all providers failed for {}: {}",
        chunk_name,
        last_err.as_deref().unwrap_or("no provider configured")
    ))
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Transcribe a URL or local file path.  Returns the joined transcript text.
///
/// `source`  – a URL (passed to yt-dlp) or a local file path.
/// `provider` – `"auto"` (groq → openai), `"groq"`, or `"openai"`.
/// `config`   – optional config; when `None` uses `Config::default()`.
pub fn transcribe(
    source: &str,
    provider: &str,
    config: Option<&Config>,
) -> Result<String, String> {
    let default_cfg;
    let cfg: &Config = match config {
        Some(c) => c,
        None => {
            default_cfg = Config::default();
            &default_cfg
        }
    };

    let order = provider_order(provider)?;

    // Fail fast — at least one provider must have a key before we do
    // expensive I/O (download / encode).
    if !order.iter().any(|p| get_provider_key(p, cfg).is_some()) {
        let names: Vec<&str> = order
            .iter()
            .filter_map(|p| get_provider(*p).ok().map(|info| info.key_field))
            .collect();
        return Err(format!(
            "no provider key configured (need one of: {})",
            names.join(", ")
        ));
    }

    // --- temporary workspace ---
    let tmp = TempDir::new().map_err(|e| format!("cannot create temp dir: {}", e))?;
    let work_dir = tmp.path();

    // --- acquire audio ---
    let src_path = Path::new(source);
    let audio = if src_path.is_file() {
        src_path.to_path_buf()
    } else {
        download_audio(source, work_dir)?
    };

    // --- compress ---
    let compressed = compress_audio(&audio, work_dir)?;

    // --- chunk if needed ---
    let chunks: Vec<PathBuf> = match std::fs::metadata(&compressed) {
        Ok(meta) if meta.len() <= SIZE_LIMIT_BYTES => vec![compressed],
        Ok(_) => chunk_audio(&compressed, work_dir)?,
        Err(e) => return Err(format!("cannot stat compressed file: {}", e)),
    };

    // --- transcribe each chunk ---
    let mut pieces: Vec<String> = Vec::new();
    for chunk in &chunks {
        let text = transcribe_chunk_with_fallback(chunk, &order, cfg)?;
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() {
            pieces.push(trimmed);
        }
    }

    Ok(pieces.join("\n"))
}

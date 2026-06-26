//! Cookie extraction from local browser databases.
//!
//! Reads cookies directly from browser SQLite databases (Chrome, Firefox,
//! Edge, Brave, Opera). Extracts platform-specific cookies for Twitter/X,
//! XiaoHongShu, Bilibili, and Xueqiu.
//!
//! ## Encryption
//!
//! - **Windows**: Chrome-based browsers encrypt cookies with DPAPI
//!   (`CryptUnprotectData`). Values may be prefixed with "v10" or "v11";
//!   the 3-byte prefix is stripped before decryption.
//! - **macOS**: Chrome uses Keychain; we attempt plaintext first.
//! - **Linux**: Cookies are typically stored as plaintext.
//! - **Firefox**: Cookies are unencrypted on all platforms.

use crate::config::Config;
use rusqlite::Connection;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── Platform specs ────────────────────────────────────────────────────

#[allow(dead_code)]
struct PlatformSpec {
    name: &'static str,
    domains: &'static [&'static str],
    /// `None` means grab all cookies as a header string.
    cookies: Option<&'static [&'static str]>,
    config_key: &'static str,
}

const PLATFORM_SPECS: &[PlatformSpec] = &[
    PlatformSpec {
        name: "Twitter/X",
        domains: &[".x.com", ".twitter.com"],
        cookies: Some(&["auth_token", "ct0"]),
        config_key: "twitter",
    },
    PlatformSpec {
        name: "XiaoHongShu",
        domains: &[".xiaohongshu.com"],
        cookies: None,
        config_key: "xhs",
    },
    PlatformSpec {
        name: "Bilibili",
        domains: &[".bilibili.com"],
        cookies: Some(&["SESSDATA", "bili_jct"]),
        config_key: "bilibili",
    },
    PlatformSpec {
        name: "Xueqiu",
        domains: &[".xueqiu.com", "xueqiu.com"],
        cookies: None,
        config_key: "xueqiu",
    },
];

// ── Public types ──────────────────────────────────────────────────────

/// Extracted cookies for a single platform.
#[derive(Debug, Clone)]
pub enum PlatformCookies {
    /// Specific named cookies, e.g. `{"auth_token": "xxx", "ct0": "yyy"}`.
    SpecificCookies(HashMap<String, String>),
    /// Entire cookie header string: `"name1=val1; name2=val2; ..."`.
    CookieHeader(String),
}

// ── Internal types ────────────────────────────────────────────────────

struct RawCookie {
    name: String,
    value: String,
    domain: String,
}

// ── Browser DB path discovery ─────────────────────────────────────────

/// Return all cookie-database paths for a given browser name.
fn find_cookie_paths(browser: &str) -> Result<Vec<PathBuf>, String> {
    match browser.to_lowercase().as_str() {
        "chrome" => find_chromium_paths("Google", "Chrome"),
        "edge" => find_chromium_paths("Microsoft", "Edge"),
        "brave" => find_chromium_paths("BraveSoftware", "Brave-Browser"),
        "opera" => find_opera_paths(),
        "firefox" => find_firefox_paths(),
        _ => Err(format!(
            "Unsupported browser: {browser}. Supported: chrome, firefox, edge, brave, opera"
        )),
    }
}

/// Resolve the "User Data" (or equivalent) directory for a Chromium-based browser.
fn chromium_user_data_dir(company: &str, product: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
        PathBuf::from(local)
            .join(company)
            .join(product)
            .join("User Data")
    }
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().unwrap_or_default();
        home.join("Library")
            .join("Application Support")
            .join(company)
            .join(product)
    }
    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().unwrap_or_default();
        // Chrome → google-chrome, Edge → microsoft-edge, Brave → brave-browser
        let dirname = format!("{}-{}", company.to_lowercase(), product.to_lowercase());
        home.join(".config").join(dirname)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        PathBuf::new()
    }
}

/// Find cookie databases across all Chromium profiles (Default, Profile 1, …).
fn find_chromium_paths(company: &str, product: &str) -> Result<Vec<PathBuf>, String> {
    let user_data = chromium_user_data_dir(company, product);
    let mut paths = Vec::new();

    if !user_data.exists() {
        return Err(format!(
            "{product} User Data directory not found: {}",
            user_data.display()
        ));
    }

    // Scan profile directories
    if let Ok(entries) = std::fs::read_dir(&user_data) {
        for entry in entries.flatten() {
            let profile_dir = entry.path();
            if !profile_dir.is_dir() {
                continue;
            }
            let name = profile_dir.file_name().unwrap_or_default().to_string_lossy();
            // Chrome profiles: "Default", "Profile 1", "Profile 2", …
            if name != "Default" && !name.starts_with("Profile ") {
                continue;
            }
            // Windows Chrome ≥130 puts Cookies under Network/
            for sub in &["Network", ""] {
                let db = if sub.is_empty() {
                    profile_dir.join("Cookies")
                } else {
                    profile_dir.join(sub).join("Cookies")
                };
                if db.exists() {
                    paths.push(db);
                }
            }
        }
    }

    if paths.is_empty() {
        Err(format!(
            "No cookie database found for {product}. Looked in: {}",
            user_data.display()
        ))
    } else {
        Ok(paths)
    }
}

fn find_opera_paths() -> Result<Vec<PathBuf>, String> {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        let base = PathBuf::from(appdata)
            .join("Opera Software")
            .join("Opera Stable");
        for sub in &["Network", ""] {
            let db = if sub.is_empty() {
                base.join("Cookies")
            } else {
                base.join(sub).join("Cookies")
            };
            if db.exists() {
                return Ok(vec![db]);
            }
        }
        Err(format!("No Opera cookie database found in {}", base.display()))
    }
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().unwrap_or_default();
        let base = home
            .join("Library")
            .join("Application Support")
            .join("com.operasoftware.Opera");
        let db = base.join("Cookies");
        if db.exists() {
            Ok(vec![db])
        } else {
            Err(format!(
                "No Opera cookie database found in {}",
                base.display()
            ))
        }
    }
    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().unwrap_or_default();
        // Opera, Opera Beta, Opera Developer
        for name in &["opera", "opera-beta", "opera-developer"] {
            let db = home.join(".config").join(name).join("Cookies");
            if db.exists() {
                return Ok(vec![db]);
            }
        }
        Err("No Opera cookie database found".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Opera cookie extraction not supported on this platform".to_string())
    }
}

fn firefox_profiles_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(appdata)
            .join("Mozilla")
            .join("Firefox")
            .join("Profiles")
    }
    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().unwrap_or_default();
        home.join("Library")
            .join("Application Support")
            .join("Firefox")
            .join("Profiles")
    }
    #[cfg(target_os = "linux")]
    {
        let home = dirs::home_dir().unwrap_or_default();
        home.join(".mozilla").join("firefox")
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        PathBuf::new()
    }
}

fn find_firefox_paths() -> Result<Vec<PathBuf>, String> {
    let profiles_dir = firefox_profiles_dir();

    if !profiles_dir.exists() {
        return Err(format!(
            "Firefox profiles directory not found: {}",
            profiles_dir.display()
        ));
    }

    let mut paths = Vec::new();

    // Read profiles.ini to find profile directories, then look for cookies.sqlite.
    // profiles.ini lists [ProfileN] sections with Path= and IsRelative= keys.
    let ini_path = profiles_dir.join("profiles.ini");
    if ini_path.exists() {
        if let Ok(ini) = std::fs::read_to_string(&ini_path) {
            for line in ini.lines() {
                let line = line.trim();
                if let Some(profile_path) = line.strip_prefix("Path=") {
                    let cookie_db = if profile_path.starts_with('/')
                        || (profile_path.len() > 2 && &profile_path[1..2] == ":")
                    {
                        // Absolute path
                        PathBuf::from(profile_path).join("cookies.sqlite")
                    } else {
                        // Relative to profiles dir
                        profiles_dir.join(profile_path).join("cookies.sqlite")
                    };
                    if cookie_db.exists() {
                        paths.push(cookie_db);
                    }
                }
            }
        }
    }

    // Fallback: scan all subdirectories for cookies.sqlite
    if paths.is_empty() {
        if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let cookie_db = path.join("cookies.sqlite");
                    if cookie_db.exists() {
                        paths.push(cookie_db);
                    }
                }
            }
        }
    }

    if paths.is_empty() {
        Err(format!(
            "No Firefox cookie databases found in {}",
            profiles_dir.display()
        ))
    } else {
        Ok(paths)
    }
}

// ── Cookie reading from SQLite ────────────────────────────────────────

/// Read cookies from a Chromium-based browser SQLite database.
fn read_chromium_cookies(db_path: &Path) -> Result<Vec<RawCookie>, String> {
    let conn = Connection::open(db_path).map_err(|e| {
        format!(
            "Failed to open cookie database {}: {e}",
            db_path.display()
        )
    })?;

    let mut stmt = conn
        .prepare("SELECT host_key, name, encrypted_value FROM cookies")
        .map_err(|e| format!("Failed to query cookies table: {e}"))?;

    let cookies: Vec<RawCookie> = stmt
        .query_map([], |row| {
            let host_key: String = row.get(0)?;
            let name: String = row.get(1)?;
            let encrypted_value: Vec<u8> = row.get(2)?;
            Ok((host_key, name, encrypted_value))
        })
        .map_err(|e| format!("Failed to read cookie rows: {e}"))?
        .filter_map(|r| r.ok())
        .filter_map(|(host_key, name, encrypted_value)| {
            decrypt_value(&encrypted_value).map(|value| RawCookie {
                name,
                value,
                domain: host_key,
            })
        })
        .collect();

    Ok(cookies)
}

/// Read cookies from a Firefox `cookies.sqlite` database.
fn read_firefox_cookies(db_path: &Path) -> Result<Vec<RawCookie>, String> {
    let conn = Connection::open(db_path).map_err(|e| {
        format!(
            "Failed to open Firefox cookie database {}: {e}",
            db_path.display()
        )
    })?;

    let mut stmt = conn
        .prepare("SELECT host, name, value FROM moz_cookies")
        .map_err(|e| format!("Failed to query Firefox moz_cookies table: {e}"))?;

    let cookies: Vec<RawCookie> = stmt
        .query_map([], |row| {
            Ok(RawCookie {
                domain: row.get(0)?,
                name: row.get(1)?,
                value: row.get(2)?,
            })
        })
        .map_err(|e| format!("Failed to read Firefox cookie rows: {e}"))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(cookies)
}

// ── Decryption ────────────────────────────────────────────────────────

/// Attempt to decrypt an `encrypted_value` blob from the Chromium cookie store.
///
/// Tries plaintext first, then falls back to platform-specific decryption.
fn decrypt_value(encrypted_value: &[u8]) -> Option<String> {
    // 1. Plaintext (common on Linux, also works if value was never encrypted)
    if let Ok(s) = String::from_utf8(encrypted_value.to_vec()) {
        if !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_graphic() || c == ' ' || c == '%' || c == '+')
        {
            return Some(s);
        }
    }

    // 2. Platform-specific decryption
    #[cfg(target_os = "windows")]
    {
        return decrypt_windows(encrypted_value);
    }
    #[cfg(target_os = "macos")]
    {
        return decrypt_macos_cookie(encrypted_value);
    }
    #[cfg(target_os = "linux")]
    {
        // On Linux, Chromium cookies are typically plaintext. If encryption
        // (GNOME Keyring/KWallet) is in use, we cannot decrypt them.
        use std::sync::atomic::{AtomicBool, Ordering};
        static WARNED_LINUX: AtomicBool = AtomicBool::new(false);
        if !WARNED_LINUX.swap(true, Ordering::Relaxed) {
            eprintln!(
                "Warning: Linux encrypted cookie decryption is not yet supported. \
                 Encrypted Chrome/Edge/Brave cookies will be skipped. \
                 Use Firefox (plaintext cookies) or run: agent-reach configure --from-browser firefox"
            );
        }
        return None;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = encrypted_value;
        None
    }
}

#[cfg(target_os = "windows")]
fn decrypt_windows(encrypted_value: &[u8]) -> Option<String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};

    // Strip "v10" or "v11" 3-byte prefix before DPAPI decryption.
    let data: &[u8] = if encrypted_value.len() > 3
        && (encrypted_value.starts_with(b"v10") || encrypted_value.starts_with(b"v11"))
    {
        &encrypted_value[3..]
    } else {
        encrypted_value
    };

    if data.is_empty() {
        return None;
    }

    let data_in = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };

    let mut data_out = CRYPT_INTEGER_BLOB::default();

    // SAFETY: data_in points to a valid slice; data_out is stack-allocated
    // and will be filled by CryptUnprotectData. We free the output buffer
    // with LocalFree afterwards.
    let result = unsafe {
        CryptUnprotectData(
            &data_in,
            None,         // ppszDataDescr
            None,         // pOptionalEntropy
            None,         // pvReserved
            None,         // pPromptStruct
            0,            // dwFlags
            &mut data_out,
        )
    };

    if result.is_ok() {
        let decrypted =
            unsafe { std::slice::from_raw_parts(data_out.pbData, data_out.cbData as usize) };
        let s = String::from_utf8(decrypted.to_vec()).ok();
        // Free the output buffer allocated by CryptUnprotectData.
        unsafe {
            let _ = LocalFree(HLOCAL(data_out.pbData as *mut core::ffi::c_void));
        }
        s
    } else {
        log::debug!(
            "CryptUnprotectData failed for cookie value ({} bytes)",
            data.len()
        );
        None
    }
}

// ── macOS Keychain + AES-128-CBC decryption ───────────────────────────

/// Retrieve the Chrome encryption key from the macOS Keychain.
///
/// The key is cached after the first successful lookup (avoids repeated
/// `security` CLI invocations for every cookie).
///
/// Uses the `security` CLI which ships with macOS.
#[cfg(target_os = "macos")]
fn get_chrome_keychain_key() -> Option<&'static [u8]> {
    use std::sync::OnceLock;
    static KEY: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    KEY.get_or_init(|| {
        let result = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-w",
                "-s",
                "Chrome Safe Storage",
                "-a",
                "Chrome",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output();

        match result {
            Ok(output) if output.status.success() && !output.stdout.is_empty() => {
                log::info!("macOS: Retrieved Chrome encryption key from Keychain");
                Some(output.stdout)
            }
            _ => {
                log::warn!(
                    "macOS: Could not retrieve Chrome encryption key from Keychain. \
                     Make sure you have launched Chrome at least once \
                     (key is stored on first run)."
                );
                None
            }
        }
    })
    .as_deref()
}

/// Decrypt ciphertext using AES-128-CBC via the system `openssl` CLI.
///
/// `key` is truncated or zero-padded to exactly 16 bytes for AES-128.
/// Returns the raw decrypted bytes (including PKCS7 padding).
#[cfg(target_os = "macos")]
fn aes_128_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Option<Vec<u8>> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    // AES-128 requires exactly 16 bytes. Truncate or zero-pad the key.
    let mut aes_key = [0u8; 16];
    let copy_len = key.len().min(16);
    aes_key[..copy_len].copy_from_slice(&key[..copy_len]);

    let key_hex: String = aes_key.iter().map(|b| format!("{b:02x}")).collect();
    let iv_hex: String = iv.iter().map(|b| format!("{b:02x}")).collect();

    let mut child = Command::new("openssl")
        .args([
            "enc",
            "-aes-128-cbc",
            "-d",
            "-K",
            &key_hex,
            "-iv",
            &iv_hex,
            "-nopad",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    child.stdin.as_mut()?.write_all(ciphertext).ok()?;
    let output = child.wait_with_output().ok()?;

    if output.status.success() {
        Some(output.stdout)
    } else {
        log::debug!("openssl AES-128-CBC decryption failed");
        None
    }
}

/// Remove PKCS7 padding from a decrypted block.
///
/// Returns the unpadded bytes, or `None` if the padding is invalid.
#[cfg(target_os = "macos")]
fn pkcs7_unpad(data: &[u8]) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    let pad_len = *data.last().unwrap() as usize;
    // PKCS7 pads with 1–16 bytes
    if pad_len == 0 || pad_len > 16 || pad_len > data.len() {
        return None;
    }
    // Verify every padding byte equals pad_len
    for &b in &data[data.len() - pad_len..] {
        if b as usize != pad_len {
            return None;
        }
    }
    Some(data[..data.len() - pad_len].to_vec())
}

/// Decrypt a single Chrome cookie encrypted_value blob on macOS.
///
/// Format (after "v10"/"v11" prefix): IV (16 bytes) + ciphertext (AES-128-CBC, PKCS7 padded).
#[cfg(target_os = "macos")]
fn decrypt_macos_cookie(encrypted_value: &[u8]) -> Option<String> {
    // Strip "v10" or "v11" 3-byte version prefix
    let payload = if encrypted_value.len() > 3
        && (encrypted_value.starts_with(b"v10") || encrypted_value.starts_with(b"v11"))
    {
        &encrypted_value[3..]
    } else {
        encrypted_value
    };

    // Need at least 16 bytes IV + 1 byte ciphertext
    if payload.len() < 17 {
        log::debug!("macOS cookie blob too short for AES-CBC: {} bytes", payload.len());
        return None;
    }

    let iv = &payload[..16];
    let ciphertext = &payload[16..];

    let key = get_chrome_keychain_key()?;
    let padded = aes_128_cbc_decrypt(key, iv, ciphertext)?;
    let plaintext = pkcs7_unpad(&padded)?;

    String::from_utf8(plaintext).ok()
}

// ── Public API ────────────────────────────────────────────────────────

/// Extract cookies for all supported platforms from a local browser.
///
/// Returns a map from `config_key` (`"twitter"`, `"xhs"`, `"bilibili"`,
/// `"xueqiu"`) to the extracted cookies.
///
/// # Errors
///
/// Returns `Err(String)` if the browser is unsupported, the cookie database
/// cannot be found, or no cookies could be read.
pub fn extract_all(browser: &str) -> Result<HashMap<String, PlatformCookies>, String> {
    let db_paths = find_cookie_paths(browser)?;
    let browsers = browser.to_lowercase();
    let is_firefox = browsers == "firefox";

    let mut all_cookies: Vec<RawCookie> = Vec::new();

    for path in &db_paths {
        let result = if is_firefox {
            read_firefox_cookies(path)
        } else {
            read_chromium_cookies(path)
        };

        match result {
            Ok(cookies) => {
                log::info!(
                    "Read {} cookies from {}",
                    cookies.len(),
                    path.display()
                );
                all_cookies.extend(cookies);
            }
            Err(e) => {
                log::warn!("Skipping {}: {e}", path.display());
            }
        }
    }

    if all_cookies.is_empty() {
        return Err(format!(
            "No cookies could be read from {browser}. \
             Make sure the browser is closed and you are logged into the target sites."
        ));
    }

    let mut results = HashMap::new();

    for spec in PLATFORM_SPECS {
        let mut platform_cookies: HashMap<String, String> = HashMap::new();
        let mut all_for_domain: Vec<&RawCookie> = Vec::new();

        for cookie in &all_cookies {
            let domain_match = spec.domains.iter().any(|d| {
                cookie.domain.ends_with(d)
                    || cookie.domain == d.strip_prefix('.').unwrap_or(d)
            });

            if !domain_match {
                continue;
            }

            all_for_domain.push(cookie);

            if let Some(needed) = spec.cookies {
                if needed.contains(&cookie.name.as_str()) {
                    platform_cookies.insert(cookie.name.clone(), cookie.value.clone());
                }
            }
        }

        if spec.cookies.is_none() {
            // Grab all matching cookies as a `Cookie: …` header string.
            if !all_for_domain.is_empty() {
                let cookie_str = all_for_domain
                    .iter()
                    .map(|c| format!("{}={}", c.name, c.value))
                    .collect::<Vec<_>>()
                    .join("; ");
                results.insert(
                    spec.config_key.to_string(),
                    PlatformCookies::CookieHeader(cookie_str),
                );
            }
        } else if !platform_cookies.is_empty() {
            results.insert(
                spec.config_key.to_string(),
                PlatformCookies::SpecificCookies(platform_cookies),
            );
        }
    }

    Ok(results)
}

/// Extract cookies from a browser and configure all found platforms.
///
/// Returns a list of `(platform_name, success, message)` tuples suitable
/// for display to the user.
pub fn configure_from_browser(
    browser: &str,
    config: &mut Config,
) -> Vec<(String, bool, String)> {
    let extracted = match extract_all(browser) {
        Ok(e) => e,
        Err(e) => return vec![("Browser".to_string(), false, e)],
    };

    if extracted.is_empty() {
        return vec![(
            "All platforms".to_string(),
            false,
            format!(
                "No platform cookies found in {browser}. \
                 Make sure you're logged into Twitter, XiaoHongShu, etc. in {browser}."
            ),
        )];
    }

    let mut results = Vec::new();

    // ── Twitter/X ─────────────────────────────────────────────────
    if let Some(PlatformCookies::SpecificCookies(tc)) = extracted.get("twitter") {
        if let (Some(auth_token), Some(ct0)) = (tc.get("auth_token"), tc.get("ct0")) {
            let _ = config.set("twitter_auth_token", auth_token);
            let _ = config.set("twitter_ct0", ct0);
            results.push((
                "Twitter/X".to_string(),
                true,
                "auth_token + ct0".to_string(),
            ));
        } else {
            let found: Vec<_> = tc.keys().cloned().collect();
            let missing: Vec<&str> = ["auth_token", "ct0"]
                .iter()
                .filter(|k| !tc.contains_key(**k))
                .copied()
                .collect();
            results.push((
                "Twitter/X".to_string(),
                false,
                format!(
                    "Found {}, but missing: {}. \
                     Make sure you're logged into x.com in {browser}.",
                    found.join(", "),
                    missing.join(", ")
                ),
            ));
        }
    }

    // ── XiaoHongShu ───────────────────────────────────────────────
    if let Some(PlatformCookies::CookieHeader(cookie_str)) = extracted.get("xhs") {
        if !cookie_str.is_empty() {
            let _ = config.set("xhs_cookie", cookie_str);
            let n = cookie_str.split(';').count();
            results.push((
                "XiaoHongShu".to_string(),
                true,
                format!("{n} cookies"),
            ));
        }
    }

    // ── Bilibili ──────────────────────────────────────────────────
    if let Some(PlatformCookies::SpecificCookies(bc)) = extracted.get("bilibili") {
        if let Some(sessdata) = bc.get("SESSDATA") {
            let _ = config.set("bilibili_sessdata", sessdata);
            let mut msg = "SESSDATA".to_string();
            if let Some(csrf) = bc.get("bili_jct") {
                let _ = config.set("bilibili_csrf", csrf);
                msg.push_str(" + bili_jct");
            }
            results.push(("Bilibili".to_string(), true, msg));
        } else {
            results.push((
                "Bilibili".to_string(),
                false,
                format!(
                    "No SESSDATA found. \
                     Make sure you're logged into bilibili.com in {browser}."
                ),
            ));
        }
    }

    // ── Xueqiu ────────────────────────────────────────────────────
    if let Some(PlatformCookies::CookieHeader(cookie_str)) = extracted.get("xueqiu") {
        if cookie_str.contains("xq_a_token") {
            let _ = config.set("xueqiu_cookie", cookie_str);
            let n = cookie_str.split(';').count();
            results.push((
                "Xueqiu".to_string(),
                true,
                format!("{n} cookies (含 xq_a_token)"),
            ));
        } else if !cookie_str.is_empty() {
            let n = cookie_str.split(';').count();
            results.push((
                "Xueqiu".to_string(),
                false,
                format!(
                    "{n} cookies found but missing xq_a_token. \
                     Please log into xueqiu.com in {browser} first."
                ),
            ));
        }
    }

    results
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_specs_nonempty() {
        assert_eq!(PLATFORM_SPECS.len(), 4);
        for spec in PLATFORM_SPECS {
            assert!(!spec.name.is_empty());
            assert!(!spec.domains.is_empty());
            assert!(!spec.config_key.is_empty());
        }
    }

    #[test]
    fn test_extract_all_unsupported_browser() {
        let result = extract_all("nonexistent_browser");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Unsupported browser"));
    }

    #[test]
    fn test_configure_unsupported_browser() {
        let mut config = Config::default();
        let results = configure_from_browser("nonexistent_browser", &mut config);
        assert_eq!(results.len(), 1);
        assert!(!results[0].1); // success = false
        assert!(results[0].2.contains("Unsupported browser"));
    }
}

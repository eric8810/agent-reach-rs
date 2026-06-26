//! Configuration management for Agent Reach.
//!
//! Stores settings in `~/.agent-reach/config.yaml`.
//! Auto-creates directory on first use.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Feature → required config keys.
const FEATURE_REQUIREMENTS: &[(&str, &[&str])] = &[
    ("exa_search", &["exa_api_key"]),
    ("twitter_xreach", &["twitter_auth_token", "twitter_ct0"]),
    ("groq_whisper", &["groq_api_key"]),
    ("openai_whisper", &["openai_api_key"]),
    ("github_token", &["github_token"]),
];

/// Manages Agent Reach configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    config_path: PathBuf,

    /// Stores all config key-value pairs. Values are `serde_yaml::Value` to
    /// preserve typed YAML values (integers, booleans, lists) that Python may
    /// write to the shared `~/.agent-reach/config.yaml`.
    #[serde(flatten)]
    pub data: HashMap<String, serde_yaml::Value>,
}

impl Config {
    /// Default config directory: `~/.agent-reach`
    pub fn config_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".agent-reach")
    }

    /// Default config file path.
    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.yaml")
    }

    /// Load config from the default location.
    pub fn load() -> Result<Self> {
        Self::load_from(Self::config_file())
    }

    /// Load config from a specific path.
    pub fn load_from(path: PathBuf) -> Result<Self> {
        let config_dir = path.parent().unwrap().to_path_buf();
        std::fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create config dir: {}", config_dir.display()))?;

        let data: HashMap<String, serde_yaml::Value> = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config: {}", path.display()))?;
            serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse config YAML: {}", path.display()))?
        } else {
            HashMap::new()
        };

        Ok(Config { config_path: path, data })
    }

    /// Save config to YAML file with restricted permissions.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let yaml = serde_yaml::to_string(&self.data)?;

        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::PermissionsExt;
            // Create file with 0o600 (owner read/write only)
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            match opts.open(&self.config_path) {
                Ok(mut f) => {
                    // Best-effort: set permissions before writing
                    let _ = f.set_permissions(std::fs::Permissions::from_mode(0o600));
                    f.write_all(yaml.as_bytes())?;
                }
                Err(_) => {
                    // Fallback: plain write without permission restriction (matching Python)
                    std::fs::write(&self.config_path, &yaml)?;
                }
            }
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&self.config_path, &yaml)?;
        }

        Ok(())
    }

    /// Get a config value. Checks file first, then environment variable (uppercase).
    /// Returns the string representation, converting typed YAML values as needed.
    pub fn get(&self, key: &str) -> Option<String> {
        if let Some(val) = self.data.get(key) {
            return Some(value_to_string(val));
        }
        std::env::var(key.to_uppercase()).ok()
    }

    /// Set a config value and save.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        self.data.insert(key.to_string(), serde_yaml::Value::String(value.to_string()));
        self.save()
    }

    /// Delete a config key and save.
    pub fn delete(&mut self, key: &str) -> Result<()> {
        self.data.remove(key);
        self.save()
    }

    /// Check if a feature has all required config keys.
    pub fn is_configured(&self, feature: &str) -> bool {
        for (feat, required) in FEATURE_REQUIREMENTS {
            if *feat == feature {
                return required.iter().all(|k| {
                    self.get(k).map_or(false, |v| !v.is_empty())
                });
            }
        }
        false
    }

    /// Return status of all optional features.
    pub fn get_configured_features(&self) -> HashMap<String, bool> {
        FEATURE_REQUIREMENTS
            .iter()
            .map(|(feat, _)| (feat.to_string(), self.is_configured(feat)))
            .collect()
    }

    /// Return config as dict (masks sensitive values).
    pub fn to_masked_dict(&self) -> HashMap<String, String> {
        let mut masked = HashMap::new();
        for (k, v) in &self.data {
            let s = value_to_string(v);
            let sensitive = k.to_lowercase().contains("key")
                || k.to_lowercase().contains("token")
                || k.to_lowercase().contains("password")
                || k.to_lowercase().contains("proxy");
            if sensitive {
                let masked_val = if s.len() > 8 {
                    format!("{}...", &s[..8])
                } else {
                    "***".to_string()
                };
                masked.insert(k.clone(), masked_val);
            } else {
                masked.insert(k.clone(), s);
            }
        }
        masked
    }
}

impl Default for Config {
    fn default() -> Self {
        // Create a config with the default path, trying to load existing
        Self::load().unwrap_or_else(|_| Config {
            config_path: Self::config_file(),
            data: HashMap::new(),
        })
    }
}

/// Convert a `serde_yaml::Value` to its string representation.
///
/// Handles scalars (numbers, booleans, strings), and falls back to
/// YAML representation for complex types (sequences, mappings).
fn value_to_string(val: &serde_yaml::Value) -> String {
    match val {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Null => String::new(),
        // For complex types, serialize back to YAML string
        other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
    }
}

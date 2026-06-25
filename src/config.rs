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

    #[serde(flatten)]
    pub data: HashMap<String, String>,
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

        let data: HashMap<String, String> = if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config: {}", path.display()))?;
            serde_yaml::from_str(&content).unwrap_or_default()
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
            let mut f = opts.open(&self.config_path)?;
            f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            f.write_all(yaml.as_bytes())?;
        }
        #[cfg(not(unix))]
        {
            std::fs::write(&self.config_path, &yaml)?;
        }

        Ok(())
    }

    /// Get a config value. Checks file first, then environment variable (uppercase).
    pub fn get(&self, key: &str) -> Option<String> {
        if let Some(val) = self.data.get(key) {
            return Some(val.clone());
        }
        std::env::var(key.to_uppercase()).ok()
    }

    /// Set a config value and save.
    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        self.data.insert(key.to_string(), value.to_string());
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
            let sensitive = k.contains("key") || k.contains("token") || k.contains("password") || k.contains("proxy");
            if sensitive {
                let masked_val = if v.len() > 8 {
                    format!("{}...", &v[..8])
                } else {
                    "***".to_string()
                };
                masked.insert(k.clone(), masked_val);
            } else {
                masked.insert(k.clone(), v.clone());
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

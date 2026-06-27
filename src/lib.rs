//! Agent Reach — Rust port.
//!
//! Give your AI Agent eyes to see the entire internet.
//! Installer + doctor + config tool. NOT a wrapper — after install, agents call upstream tools directly.

pub mod backends;
pub mod channels;
pub mod cli;
pub mod config;
pub mod cookie_extract;
pub mod doctor;
pub mod ffmpeg_dl;
pub mod install;
pub mod mcp_server;
pub mod probe;
pub mod skill;
pub mod transcribe;
pub mod utils;

/// Current version (must match Cargo.toml).
pub const VERSION: &str = "1.5.0";

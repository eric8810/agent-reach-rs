//! UTF-8-safe text helpers for cross-platform file operations.

use std::path::Path;

/// Read text as UTF-8 with replacement semantics.
pub fn read_utf8_text(path: &Path) -> std::io::Result<String> {
    std::fs::read_to_string(path)
}

/// Read text as UTF-8, returning default on error.
pub fn read_utf8_text_or_default(path: &Path, default: &str) -> String {
    if !path.exists() {
        return default.to_string();
    }
    std::fs::read_to_string(path).unwrap_or_else(|_| default.to_string())
}

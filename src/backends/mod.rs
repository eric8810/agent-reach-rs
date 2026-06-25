//! Cross-channel backends.
//!
//! A backend here is an upstream runtime that serves MULTIPLE channels
//! (e.g. OpenCLI covers xiaohongshu/reddit/bilibili/twitter through one
//! browser session), as opposed to the per-platform tools probed inside
//! each channel file.

pub mod opencli;

pub use opencli::{opencli_status, opencli_summary, OpenCLIStatus};

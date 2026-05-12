//! cc-ui: Rust terminal UI extraction boundary.
//!
//! Extracted Rust TUI source files live here while `claude-code-rs::ui` keeps a
//! root facade for stable `crate::ui::*` imports during the workspace split.

#![forbid(unsafe_code)]

/// Transitional marker for the Rust UI extraction boundary.
pub const EXTRACTION_BOUNDARY: &str = "cc-ui";

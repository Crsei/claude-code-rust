//! cc-ui: Rust terminal UI extraction boundary.
//!
//! The active Rust TUI still lives under `claude-code-rs::ui` during this
//! scaffold step. Follow-up extraction tasks move entry modules here while the
//! binary keeps a root facade for stable `crate::ui::*` imports.

#![forbid(unsafe_code)]

/// Transitional marker for the Rust UI extraction boundary.
pub const EXTRACTION_BOUNDARY: &str = "cc-ui";

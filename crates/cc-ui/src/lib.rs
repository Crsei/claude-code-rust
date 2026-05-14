//! cc-ui: placeholder Rust terminal UI extraction boundary.
//!
//! Rust TUI source is intentionally owned by `claude-code-rs::ui` for now.
//! This crate remains empty so a future extraction can be reintroduced without
//! changing the workspace manifest shape.

#![forbid(unsafe_code)]

/// Transitional marker for the Rust UI extraction boundary.
pub const EXTRACTION_BOUNDARY: &str = "cc-ui";

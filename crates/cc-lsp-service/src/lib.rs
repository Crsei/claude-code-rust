//! Standalone Language Server Protocol service crate.
//!
//! This crate owns LSP server config resolution, client lifecycle, protocol
//! conversion helpers, and the cc-tools adapter for the model-facing LSP tool.

#[path = "mod.rs"]
mod service;

pub use service::*;

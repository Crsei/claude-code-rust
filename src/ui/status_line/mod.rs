//! Scriptable status-line subsystem.
//!
//! Implements the spec in `customize-status-line.md` + issue #11:
//!
//! 1. `payload` — [`StatusLinePayload`] and its nested structs (the JSON
//!    piped to the user's command on stdin).
//! 2. `runner` — [`StatusLineRunner`], the throttled / cancellable
//!    subprocess driver.
//!
//! The default terminal footer (see [`super::app::App::render_status_bar`])
//! is always used as the fallback when no custom statusLine is configured
//! or when the script errors / times out.

pub mod payload;
pub mod runner;

// Re-export the typed building blocks. `VimStatus` is reachable via
// `payload::VimStatus` for the future wire-up of vim-mode reporting;
// pulling it up front is left out deliberately so the active surface
// stays small.
pub use payload::{ContextWindowStatus, CostStatus, ModelInfo, StatusLinePayload, WorkspaceStatus};
pub use runner::{StatusLineOutput, StatusLineRunner};

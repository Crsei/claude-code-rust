//! Engine-level shared types.
//!
//! Moved from `crates/claude-code-rs/src/types/{app_state, tool, config}.rs`
//! in Phase 6. The pure leaf types (`message`, `state`, `transitions`,
//! `permissions`, `hooks`, `commands`, `agent_*`, `background_agents`,
//! `teams`) live in `cc-types`; the three modules here depend on them plus a
//! handful of runtime-bound sibling crates (`cc-keybindings`, `cc-config`) and
//! this crate's own `status_line` module.
//!
//! The root crate re-exports these via `src/types/mod.rs` so existing
//! `crate::types::{app_state, tool, config}` import paths keep working.

pub mod app_state;
pub mod config;
pub mod tool;

// Re-export the pure-data modules from cc-types so a consumer doing
// `use cc_engine::types::message::*` or `use cc_engine::types::state::*`
// lines up with the pre-move shape of `crate::types::*`.
pub use cc_types::{message, permissions, state, transitions};

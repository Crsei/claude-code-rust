//! cc-engine — QueryEngine, Agent tool, query-loop driver, and engine-level
//! shared state (Phase 6 — in progress).
//!
//! Issue #75 (`[workspace-split] Phase 6`): this crate is the target
//! destination for `crates/claude-code-rs/src/engine/` plus the engine-level
//! shared structures (`AppState`, `Tool` trait, `ToolUseContext`) and the
//! status-line runner.
//!
//! ## What lives here now
//!
//! - [`status_line`] — scriptable status-line payload + runner, moved from
//!   `src/ui/status_line/`. Lives here because `AppState` holds a
//!   `StatusLineRunner` handle.
//!
//! ## Coming in follow-up PRs
//!
//! - `types::{tool, app_state, config}` from the root crate
//! - The contents of `src/engine/` and `src/query/`
//!
//! See `docs/superpowers/specs/2026-04-20-workspace-split-design.md`.

pub mod status_line;
pub mod types;

// Re-export from cc-types so consumers can eventually write
// `use cc_engine::{HookRunner, CommandDispatcher}` once the engine types
// land here too.
#[allow(unused_imports)]
pub use cc_types::hooks::{HookRunner, NoopHookRunner};

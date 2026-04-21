//! cc-engine — QueryEngine, Agent tool, and query-loop driver (Phase 6 scaffold).
//!
//! Issue #75 (`[workspace-split] Phase 6`): this crate is the target destination
//! for `crates/claude-code-rs/src/engine/`. The current PR publishes the crate
//! scaffold so the workspace manifest is complete; the actual source move is
//! staged in a follow-up once the last cycle-breaking work in `cc-types`
//! lands (specifically: hoisting `types/tool.rs`, `types/app_state.rs`, and
//! `types/config.rs` into cc-types, which unblocks every downstream crate
//! that touches `ToolUseContext` / `AppState` / `QueryEngineConfig`).
//!
//! See `docs/superpowers/specs/2026-04-20-workspace-split-design.md` for the
//! full extraction plan.

// Re-export from cc-types for downstream consumers of the engine hook trait
// so they can `use cc_engine::HookRunner` once the move completes.
#[allow(unused_imports)]
pub use cc_types::hooks::{HookRunner, NoopHookRunner};

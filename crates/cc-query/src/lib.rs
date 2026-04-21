//! cc-query — async streaming query loop (Phase 6 scaffold).
//!
//! Issue #75 (`[workspace-split] Phase 6`): target destination for
//! `crates/claude-code-rs/src/query/`. The current PR publishes the crate
//! scaffold so the workspace manifest lists every Phase 6/7 crate up-front.
//!
//! Cycle-breaking prerequisites (done in this PR):
//! - `query -> tools` edge removed: query now calls hooks via the
//!   `cc_types::hooks::HookRunner` trait exposed through `QueryDeps::hook_runner()`.
//! - `CompletedBackgroundAgent` / `PendingBackgroundResults` moved to
//!   `cc-types::background_agents`.
//!
//! Remaining before the source move: hoist `types/tool.rs`, `types/app_state.rs`,
//! and `types/config.rs` into cc-types so `QueryDeps` no longer touches root
//! crate items (`crate::types::{app_state, config, tool}`).

#[allow(unused_imports)]
pub use cc_types::background_agents::{CompletedBackgroundAgent, PendingBackgroundResults};

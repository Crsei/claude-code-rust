//! cc-tools — tool implementations (Phase 6 scaffold).
//!
//! Issue #75 (`[workspace-split] Phase 6`): target destination for
//! `crates/claude-code-rs/src/tools/` minus the three cycle-causing tool files
//! that move to their natural homes:
//!
//! - `tools/lsp.rs` -> cc-lsp-service (with the LSP client)
//! - `tools/send_message.rs` + `tools/team_spawn.rs` -> cc-teams
//! - `tools/system_status.rs` -> cc-ipc (with subsystem_handlers)
//!
//! Cycle-breaking prerequisites (done in this PR):
//! - `tools -> engine` removed in Phase 5 (agent tool moved to cc-engine).
//! - `background_agents` types moved to cc-types::background_agents.
//!
//! Remaining before the source move: hoist `types/tool.rs` (Tool trait,
//! ToolUseContext) to cc-types so every tool module can depend only on
//! cc-types instead of the root crate.

#[allow(unused_imports)]
pub use cc_types::background_agents::{CompletedBackgroundAgent, PendingBackgroundResults};

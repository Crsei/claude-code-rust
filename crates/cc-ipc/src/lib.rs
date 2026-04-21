//! cc-ipc — IPC spine between backend and frontend (Phase 7 scaffold).
//!
//! Issue #76 (`[workspace-split] Phase 7`): target destination for
//! `crates/claude-code-rs/src/ipc/` plus the `tools/system_status.rs` wrapper
//! (which reads `ipc::subsystem_handlers` and therefore moves with the IPC
//! crate to avoid a `cc-tools -> cc-ipc` edge).
//!
//! Cycle-breaking prerequisites (done in this PR): moved agent event / command
//! / channel types from `src/ipc/` to `cc-types::{agent_events,
//! agent_types, agent_channel}`. The files under `src/ipc/` are now thin
//! re-exports so downstream consumers don't break while the physical code
//! move is staged in a follow-up PR.

#[allow(unused_imports)]
pub use cc_types::{
    agent_channel::{agent_channel, AgentIpcEvent, AgentReceiver, AgentSender},
    agent_events::{AgentCommand, AgentEvent, TeamCommand, TeamEvent},
    agent_types::{AgentInfo, AgentNode, TeamMemberInfo},
};

//! Agent IPC channel — the dedicated mpsc channel for agent + team events.
//!
//! The real definitions now live in `cc_types::agent_channel`; this module is
//! a thin re-export so existing `crate::ipc::agent_channel::*` paths keep
//! working.

#[allow(unused_imports)]
pub use cc_types::agent_channel::{agent_channel, AgentIpcEvent, AgentReceiver, AgentSender};

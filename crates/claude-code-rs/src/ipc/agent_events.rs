//! Agent and team event/command enums for IPC.
//!
//! The real definitions now live in `cc_types::agent_events`; this module is a
//! thin re-export so existing `crate::ipc::agent_events::*` paths keep working.

#[allow(unused_imports)]
pub use cc_types::agent_events::{AgentCommand, AgentEvent, TeamCommand, TeamEvent};

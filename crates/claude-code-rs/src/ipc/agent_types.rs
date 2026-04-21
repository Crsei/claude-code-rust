//! Shared data types for agent tree, agent info, and team member IPC messages.
//!
//! The real definitions now live in `cc_types::agent_types`; this module is a
//! thin re-export so existing `crate::ipc::agent_types::*` paths keep working.

#[allow(unused_imports)]
pub use cc_types::agent_types::{AgentInfo, AgentNode, TeamMemberInfo};

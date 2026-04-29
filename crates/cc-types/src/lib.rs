//! Pure leaf types extracted from `claude-code-rs::types`.
//!
//! This crate holds the subset of `src/types/` with no cross-module dependencies
//! on the main crate (teams / ui / config / ipc). The three modules below are
//! the truly pure leaves; `app_state`, `tool`, and `config` remain in the root
//! crate because they still reach into the not-yet-extracted subsystems.
//!
//! See issue #70 (`[workspace-split] Phase 1`) for the rationale behind this
//! partial split.
pub mod agent_channel;
pub mod agent_events;
pub mod agent_types;
pub mod background_agents;
pub mod commands;
pub mod hooks;
pub mod message;
pub mod permissions;
pub mod plan_workflow;
pub mod state;
pub mod teams;
pub mod transitions;

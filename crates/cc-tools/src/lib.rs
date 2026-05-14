//! cc-tools: shared tool specs and pure registry helpers.
//!
//! This crate owns tool contracts and tool-facing implementations that can run
//! without depending on the engine, query loop, UI, or daemon crate. Domain
//! tools with heavier ownership boundaries remain in their target crates until
//! their dependencies can move cleanly.

pub mod ask_user;
pub mod brief;
pub mod config_tool;
pub mod exec;
pub mod fs;
pub mod hooks;
pub mod observable_input;
pub mod plan_workflow;
pub mod registry;
pub mod result;
pub mod send_user_message;
pub mod sleep;
pub mod structured_output;
pub mod system_status;
pub mod task_specs;
pub mod tool;
pub mod tool_search;
pub mod web_fetch;
pub mod web_search;

//! cc-engine — QueryEngine, Agent tool, query-loop driver, and engine-level
//! shared state (Phase 6 — in progress).
//!
//! Issue #75 (`[workspace-split] Phase 6`): this crate is the target
//! destination for `crates/claude-code-rs/src/engine/` plus the engine-level
//! shared structures (`AppState`, `Tool` trait, `ToolUseContext`) and the
//! status-line runner.
//!
//! See `docs/superpowers/specs/2026-04-20-workspace-split-design.md`.

extern crate self as cc_engine;

pub mod agent;
pub mod agent_runtime;
pub mod codex_exec;
pub mod command_runtime;
pub mod effort;
pub mod hooks;
pub mod input_processing;
pub mod ipc_compat;
pub mod lifecycle;
pub mod mcp_tool_adapter;
pub mod output_style;
pub mod prompt_sections;
pub mod query;
pub mod result;
pub mod services;
pub mod skill_tool;
pub mod status_line;
pub mod system_prompt;
pub mod tool_runtime;
pub mod tools;
pub mod types;
pub mod worktree_hooks;

pub use cc_bootstrap as bootstrap;
pub use cc_compact as compact;
pub use cc_config as config;
pub use cc_observability as observability;
pub use cc_permissions as permissions;
pub use cc_sandbox as sandbox;
pub use cc_session as session;
pub use cc_skills as skills;
pub use cc_utils as utils;

// Re-export from cc-types so consumers can eventually write
// `use cc_engine::{HookRunner, CommandDispatcher}` once the engine types
// land here too.
pub use cc_types::hooks::{HookRunner, NoopHookRunner};

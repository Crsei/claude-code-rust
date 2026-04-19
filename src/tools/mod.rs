//! Tool system entry point.
//!
//! Tools are grouped by domain. Each sub-domain module exposes a `tools()`
//! aggregator that returns the tools it owns, and `registry.rs` concatenates
//! those aggregators into the global tool list. This keeps `registry.rs`
//! decoupled from individual tool implementations, so adding a new tool
//! touches only its sub-domain's `mod.rs`.
//!
//! Placement rules for new tools live in `src/tools/ARCHITECTURE.md`.

// --- Domain sub-modules ------------------------------------------------------
//
// Filesystem: read, write, search files on the local disk.
pub mod fs;
// Execution: spawn subprocesses, drive time, run embedded runtimes.
pub mod exec;

// --- Infrastructure ----------------------------------------------------------
//
// Shared tool-execution machinery (permission pipeline, hook dispatch, etc.)
// Not tools themselves.
pub mod execution;
pub mod hooks;
pub mod orchestration;
pub mod registry;

// --- Single-tool / small-cluster modules -------------------------------------
//
// Not yet grouped into a sub-domain. Keep this list short — once a new
// adjacent tool appears, promote the pair into a proper sub-domain instead of
// stacking here.
pub mod ask_user;
pub mod skill;

// Agent sub-domain (already grouped).
pub mod agent;
// Background agent types (used by Agent tool + query loop + event loop).
pub mod background_agents;

// Web / network tools.
pub mod web_fetch;
pub mod web_search;

// Plan mode + Task tools.
pub mod plan_mode;
pub mod tasks;

// Worktree tools.
pub mod worktree;

// Code intelligence.
pub mod lsp;

// Inter-agent messaging (Teams).
pub mod send_message;

// Meta / UX tools.
pub mod config_tool;
pub mod send_user_message;
pub mod structured_output;

// Kairos Brief mode.
pub mod brief;

// SystemStatus (agent subsystem observability).
pub mod system_status;

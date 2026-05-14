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
// --- Infrastructure ----------------------------------------------------------
//
pub mod registry;

// --- Single-tool / small-cluster modules -------------------------------------
//
// Not yet grouped into a sub-domain. Keep this list short — once a new
// adjacent tool appears, promote the pair into a proper sub-domain instead of
// stacking here.
pub mod skill;

// Web / network tools.

// Task tools.
pub mod tasks;

// Worktree tools.
pub mod worktree;

// Code intelligence.
pub mod lsp;

// Inter-agent messaging (Teams).
pub mod pr_activity;
pub mod send_message;
pub mod team_spawn;

// Meta / UX tools.

// Kairos Brief mode.

// SystemStatus (agent subsystem observability).

// Tool discovery / retrieval.

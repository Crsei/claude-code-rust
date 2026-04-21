//! MCP (Model Context Protocol) — thin facade over `cc-mcp`.
//!
//! Phase 3 (issue #72) moved all protocol / client / transport / discovery /
//! manager code into the `cc-mcp` workspace crate. The only piece that
//! remains here is [`tools`] — the adapter that exposes MCP tools through
//! the root crate's `Tool` trait — because `Tool` (and its
//! `ToolUseContext`) still lives in the root crate and will only move out
//! once Phase 5 breaks the hub cycles.
//!
//! The `pub use cc_mcp::*;` re-export preserves every historical
//! `crate::mcp::...` path: `crate::mcp::client::McpClient`,
//! `crate::mcp::discovery::discover_mcp_servers`, etc. continue to resolve
//! unchanged.

pub use cc_mcp::*;

pub mod tools;

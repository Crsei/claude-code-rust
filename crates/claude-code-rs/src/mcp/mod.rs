//! MCP (Model Context Protocol) root adapters.
//!
//! Phase 3 (issue #72) moved all protocol / client / transport / discovery /
//! manager code into the `cc-mcp` workspace crate. The remaining root piece is
//! [`tools`], the adapter that exposes MCP tools through the root Tool list.

pub mod tools;

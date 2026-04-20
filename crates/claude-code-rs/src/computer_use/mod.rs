//! Computer Use — native desktop control tools.
//!
//! Provides detection/classification for external MCP tools (`detection`),
//! platform-native backends (`screenshot`, `input`), Tool trait wrappers
//! (`tools`), and CLI registration (`setup`).
//!
//! Reserved tool name prefix: `mcp__computer-use__*`

pub mod detection;
pub mod input;
pub mod screenshot;
pub mod setup;
pub mod tools;

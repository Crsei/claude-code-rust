//! Computer Use — native desktop control tools.
//!
//! Provides detection/classification for external MCP tools (`detection`),
//! platform-native backends (`screenshot`, `input`), Tool trait wrappers
//! (`tools`), and CLI registration (`setup`).
//!
//! Reserved tool name prefix: `mcp__computer-use__*`
//!
//! Platform-native `input` and `screenshot` backends live in `cc-computer-use`;
//! root keeps only detection, setup, and Tool trait wrappers.

pub mod detection;
pub mod setup;
pub mod tools;

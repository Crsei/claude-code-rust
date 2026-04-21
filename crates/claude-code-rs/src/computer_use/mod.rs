//! Computer Use — native desktop control tools.
//!
//! Provides detection/classification for external MCP tools (`detection`),
//! platform-native backends (`screenshot`, `input`), Tool trait wrappers
//! (`tools`), and CLI registration (`setup`).
//!
//! Reserved tool name prefix: `mcp__computer-use__*`
//!
//! Phase 3 (issue #72) moved the `input` and `screenshot` platform
//! submodules into the `cc-computer-use` workspace crate. Re-exporting them
//! here keeps every `crate::computer_use::{input,screenshot}::…` path
//! resolving for call sites in `detection`, `setup`, and `tools`.

pub use cc_computer_use::{input, screenshot};

pub mod detection;
pub mod setup;
pub mod tools;

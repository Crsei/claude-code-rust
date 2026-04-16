//! Services module — background and utility services for cc-rust.
//!
//! Provides tool-use summarization, session memory extraction,
//! prompt suggestions, and LSP server lifecycle management.

pub mod lsp_lifecycle;
pub mod prompt_suggestion;
pub mod session_memory;
pub mod tool_use_summary;

//! Shared IPC protocol DTOs.

pub mod lsp;
pub mod protocol;
pub mod subsystem_events;
pub mod subsystem_types;

pub use lsp::{CompletionItemInfo, DocumentChange, SourceRange};
pub use protocol::{
    BackendMessage, ConversationMessage, FileSearchMatch, FrontendMessage, ToolResultContentInfo,
};

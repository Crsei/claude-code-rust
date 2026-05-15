//! Shared LSP result types used by both the LSP service (this crate) and the
//! `Lsp` tool wrapper in `lsp_service::tool`.
//!
//! Previously these lived in `tools::lsp`, which produced a
//! `lsp_service -> tools` edge that blocked Phase 6 crate extraction. Moving
//! them here reverses the direction: `lsp_service::tool` now imports from
//! `lsp_service::types`, which is the natural direction given the tool is a
//! thin wrapper over the service.

use serde::Serialize;

pub use cc_ipc_protocol::{CompletionItemInfo, DocumentChange};

/// A location in a source file (simplified LSP Location).
#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub file_path: String,
    pub line: u32,      // 1-based
    pub character: u32, // 1-based
    pub end_line: Option<u32>,
    pub end_character: Option<u32>,
}

/// A symbol in a document.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub location: SourceLocation,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SymbolInfo>,
}

/// Hover information.
#[derive(Debug, Clone, Serialize)]
pub struct HoverInfo {
    pub contents: String,
    pub range: Option<SourceLocation>,
}

/// Snapshot of one live document tracked by an LSP server.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentSyncState {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
}

//! Shared LSP result types used by both the LSP service (this crate) and the
//! `Lsp` tool wrapper in `tools::lsp`.
//!
//! Previously these lived in `tools::lsp`, which produced a
//! `lsp_service -> tools` edge that blocked Phase 6 crate extraction. Moving
//! them here reverses the direction: `tools::lsp` now imports from
//! `lsp_service::types`, which is the natural direction given the tool is a
//! thin wrapper over the service.

use serde::{Deserialize, Serialize};

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

/// A range edit sent by a live editor.
///
/// Coordinates are 1-based to match the rest of the Rust-facing LSP surface.
/// They are converted back to 0-based UTF-16 LSP positions before being sent
/// to the language server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChange {
    pub range: SourceRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range_length: Option<u32>,
    pub text: String,
}

/// A source range without a file path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Snapshot of one live document tracked by an LSP server.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentSyncState {
    pub uri: String,
    pub language_id: String,
    pub version: i32,
}

/// Simplified completion item exposed to tools and IPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompletionItemInfo {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insert_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_text: Option<String>,
}

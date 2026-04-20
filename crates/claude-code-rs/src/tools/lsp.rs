//! LSP tool — code intelligence via Language Server Protocol.
//!
//! Corresponds to TypeScript: tools/LSPTool/LSPTool.ts
//!
//! Provides nine code navigation operations through a unified interface:
//! - goToDefinition, goToImplementation
//! - findReferences
//! - hover
//! - documentSymbol, workspaceSymbol
//! - prepareCallHierarchy, incomingCalls, outgoingCalls
//!
//! The tool converts 1-based editor coordinates to 0-based LSP protocol
//! coordinates and formats results for the model.
//!
//! Depends on the `lsp-types` crate.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

// ---------------------------------------------------------------------------
// LSP operations
// ---------------------------------------------------------------------------

/// Supported LSP operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspOperation {
    GoToDefinition,
    GoToImplementation,
    FindReferences,
    Hover,
    DocumentSymbol,
    WorkspaceSymbol,
    PrepareCallHierarchy,
    IncomingCalls,
    OutgoingCalls,
}

impl LspOperation {
    /// Parse an operation name from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "goToDefinition" => Some(Self::GoToDefinition),
            "goToImplementation" => Some(Self::GoToImplementation),
            "findReferences" => Some(Self::FindReferences),
            "hover" => Some(Self::Hover),
            "documentSymbol" => Some(Self::DocumentSymbol),
            "workspaceSymbol" => Some(Self::WorkspaceSymbol),
            "prepareCallHierarchy" => Some(Self::PrepareCallHierarchy),
            "incomingCalls" => Some(Self::IncomingCalls),
            "outgoingCalls" => Some(Self::OutgoingCalls),
            _ => None,
        }
    }

    /// Get the LSP method name for this operation.
    #[allow(dead_code)]
    pub fn method(&self) -> &'static str {
        match self {
            Self::GoToDefinition => "textDocument/definition",
            Self::GoToImplementation => "textDocument/implementation",
            Self::FindReferences => "textDocument/references",
            Self::Hover => "textDocument/hover",
            Self::DocumentSymbol => "textDocument/documentSymbol",
            Self::WorkspaceSymbol => "workspace/symbol",
            Self::PrepareCallHierarchy => "textDocument/prepareCallHierarchy",
            Self::IncomingCalls => "callHierarchy/incomingCalls",
            Self::OutgoingCalls => "callHierarchy/outgoingCalls",
        }
    }

    /// Whether this operation requires file position (line + character).
    pub fn requires_position(&self) -> bool {
        !matches!(self, Self::WorkspaceSymbol)
    }

    /// All valid operation names.
    pub fn all_names() -> &'static [&'static str] {
        &[
            "goToDefinition",
            "goToImplementation",
            "findReferences",
            "hover",
            "documentSymbol",
            "workspaceSymbol",
            "prepareCallHierarchy",
            "incomingCalls",
            "outgoingCalls",
        ]
    }
}

// ---------------------------------------------------------------------------
// LSP location types
// ---------------------------------------------------------------------------

/// A location in a source file (simplified LSP Location).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceLocation {
    pub file_path: String,
    pub line: u32,      // 1-based
    pub character: u32, // 1-based
    pub end_line: Option<u32>,
    pub end_character: Option<u32>,
}

/// A symbol in a document.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    pub location: SourceLocation,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<SymbolInfo>,
}

/// Hover information.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HoverInfo {
    pub contents: String,
    pub range: Option<SourceLocation>,
}

// ---------------------------------------------------------------------------
// Result formatting
// ---------------------------------------------------------------------------

fn format_locations(locations: &[SourceLocation]) -> String {
    if locations.is_empty() {
        return "No results found.".to_string();
    }

    let mut lines = Vec::new();
    // Group by file
    let mut by_file: std::collections::HashMap<&str, Vec<&SourceLocation>> =
        std::collections::HashMap::new();
    for loc in locations {
        by_file.entry(&loc.file_path).or_default().push(loc);
    }

    for (file, locs) in &by_file {
        lines.push(format!("**{}**", file));
        for loc in locs {
            lines.push(format!("  Line {}, Col {}", loc.line, loc.character));
        }
    }
    lines.join("\n")
}

fn format_symbols(symbols: &[SymbolInfo], indent: usize) -> String {
    let mut lines = Vec::new();
    let prefix = "  ".repeat(indent);
    for sym in symbols {
        lines.push(format!(
            "{}{} `{}` (line {})",
            prefix, sym.kind, sym.name, sym.location.line
        ));
        if !sym.children.is_empty() {
            lines.push(format_symbols(&sym.children, indent + 1));
        }
    }
    lines.join("\n")
}

fn format_hover(hover: &HoverInfo) -> String {
    if hover.contents.is_empty() {
        return "No hover information available.".to_string();
    }
    hover.contents.clone()
}

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

/// Maximum file size for LSP operations (10 MB).
const MAX_LSP_FILE_SIZE: u64 = 10_000_000;

pub struct LspTool;

#[async_trait]
impl Tool for LspTool {
    fn name(&self) -> &str {
        "LSP"
    }

    async fn description(&self, _: &Value) -> String {
        "Code intelligence via Language Server Protocol.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": LspOperation::all_names(),
                    "description": "The LSP operation to perform."
                },
                "filePath": {
                    "type": "string",
                    "description": "Absolute or relative path to the file."
                },
                "line": {
                    "type": "number",
                    "description": "Line number (1-based, as shown in editors)."
                },
                "character": {
                    "type": "number",
                    "description": "Column number (1-based, as shown in editors)."
                }
            },
            "required": ["operation", "filePath"]
        })
    }

    fn is_concurrency_safe(&self, _: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _: &Value) -> bool {
        true
    }

    fn is_enabled(&self) -> bool {
        true
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let op_str = input
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let Some(op) = LspOperation::from_str(op_str) else {
            return ValidationResult::Error {
                message: format!(
                    "Invalid operation '{}'. Must be one of: {}",
                    op_str,
                    LspOperation::all_names().join(", ")
                ),
                error_code: 400,
            };
        };

        let file_path = input.get("filePath").and_then(|v| v.as_str()).unwrap_or("");
        if file_path.is_empty() {
            return ValidationResult::Error {
                message: "filePath is required".to_string(),
                error_code: 400,
            };
        }

        if op.requires_position() {
            let line = input.get("line").and_then(|v| v.as_u64());
            let character = input.get("character").and_then(|v| v.as_u64());
            if line.is_none() || character.is_none() {
                // documentSymbol doesn't need position
                if op != LspOperation::DocumentSymbol {
                    return ValidationResult::Error {
                        message: format!(
                            "Operation '{}' requires line and character parameters",
                            op_str
                        ),
                        error_code: 400,
                    };
                }
            }
        }

        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let op_str = input
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let op = LspOperation::from_str(op_str).context("Invalid LSP operation")?;

        let file_path = input.get("filePath").and_then(|v| v.as_str()).unwrap_or("");

        // Convert 1-based editor coords to 0-based LSP coords
        let line = input
            .get("line")
            .and_then(|v| v.as_u64())
            .map(|l| l.saturating_sub(1) as u32)
            .unwrap_or(0);
        let character = input
            .get("character")
            .and_then(|v| v.as_u64())
            .map(|c| c.saturating_sub(1) as u32)
            .unwrap_or(0);

        // Resolve file path
        let resolved = resolve_file_path(file_path, ctx);
        if !resolved.exists() {
            return Ok(ToolResult {
                data: json!({
                    "error": format!("File not found: {}", resolved.display()),
                    "operation": op_str,
                }),
                new_messages: vec![],
                ..Default::default()
            });
        }

        // Check file size
        if let Ok(meta) = std::fs::metadata(&resolved) {
            if meta.len() > MAX_LSP_FILE_SIZE {
                return Ok(ToolResult {
                    data: json!({
                        "error": format!(
                            "File too large ({} bytes, limit {} bytes)",
                            meta.len(),
                            MAX_LSP_FILE_SIZE
                        ),
                        "operation": op_str,
                    }),
                    new_messages: vec![],
                    ..Default::default()
                });
            }
        }

        // ---- LSP execution ----
        let result = execute_lsp_operation(op, &resolved, line, character).await;

        match result {
            Ok(output) => Ok(ToolResult {
                data: output,
                new_messages: vec![],
                ..Default::default()
            }),
            Err(e) => Ok(ToolResult {
                data: json!({
                    "error": format!("LSP operation failed: {}", e),
                    "operation": op_str,
                    "filePath": resolved.display().to_string(),
                }),
                new_messages: vec![],
                ..Default::default()
            }),
        }
    }

    async fn prompt(&self) -> String {
        r#"Provides code intelligence features via Language Server Protocol (LSP).

Supported operations:
- goToDefinition: Find where a symbol is defined
- goToImplementation: Find implementations of an interface/trait
- findReferences: Find all references to a symbol
- hover: Get type information and documentation
- documentSymbol: List all symbols in a file
- workspaceSymbol: Search for symbols across the workspace
- prepareCallHierarchy: Get the call hierarchy for a function
- incomingCalls: Find callers of a function
- outgoingCalls: Find functions called by a function

All operations require filePath. Most require line and character (1-based).
Requires LSP servers to be configured for the file type."#
            .to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(op) = input
            .and_then(|v| v.get("operation"))
            .and_then(|v| v.as_str())
        {
            return format!("LSP({})", op);
        }
        "LSP".to_string()
    }
}

// ---------------------------------------------------------------------------
// File path resolution
// ---------------------------------------------------------------------------

fn resolve_file_path(file_path: &str, ctx: &ToolUseContext) -> PathBuf {
    let p = Path::new(file_path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        // Try to get CWD from app state
        let _state = (ctx.get_app_state)();
        // Fallback to current dir
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(file_path)
    }
}

// ---------------------------------------------------------------------------
// LSP execution
// ---------------------------------------------------------------------------

async fn execute_lsp_operation(
    op: LspOperation,
    file_path: &Path,
    line: u32,
    character: u32,
) -> Result<Value> {
    use crate::lsp_service;

    let uri = file_path_to_uri(file_path);

    match op {
        LspOperation::GoToDefinition => {
            let locations = lsp_service::go_to_definition(&uri, line, character).await?;
            Ok(json!({
                "operation": "goToDefinition",
                "filePath": file_path.display().to_string(),
                "result": format_locations(&locations),
                "resultCount": locations.len(),
            }))
        }
        LspOperation::GoToImplementation => {
            let locations = lsp_service::go_to_implementation(&uri, line, character).await?;
            Ok(json!({
                "operation": "goToImplementation",
                "filePath": file_path.display().to_string(),
                "result": format_locations(&locations),
                "resultCount": locations.len(),
            }))
        }
        LspOperation::FindReferences => {
            let locations = lsp_service::find_references(&uri, line, character).await?;
            let file_count = locations
                .iter()
                .map(|l| &l.file_path)
                .collect::<std::collections::HashSet<_>>()
                .len();
            Ok(json!({
                "operation": "findReferences",
                "filePath": file_path.display().to_string(),
                "result": format_locations(&locations),
                "resultCount": locations.len(),
                "fileCount": file_count,
            }))
        }
        LspOperation::Hover => {
            let hover = lsp_service::hover(&uri, line, character).await?;
            Ok(json!({
                "operation": "hover",
                "filePath": file_path.display().to_string(),
                "result": format_hover(&hover),
            }))
        }
        LspOperation::DocumentSymbol => {
            let symbols = lsp_service::document_symbols(&uri).await?;
            Ok(json!({
                "operation": "documentSymbol",
                "filePath": file_path.display().to_string(),
                "result": format_symbols(&symbols, 0),
                "resultCount": symbols.len(),
            }))
        }
        LspOperation::WorkspaceSymbol => {
            // For workspace symbol, use the file_path as a query hint
            let query = file_path.to_string_lossy();
            let symbols = lsp_service::workspace_symbols(&query).await?;
            Ok(json!({
                "operation": "workspaceSymbol",
                "result": format_symbols(&symbols, 0),
                "resultCount": symbols.len(),
            }))
        }
        LspOperation::PrepareCallHierarchy => {
            let items = lsp_service::prepare_call_hierarchy(&uri, line, character).await?;
            Ok(json!({
                "operation": "prepareCallHierarchy",
                "filePath": file_path.display().to_string(),
                "result": format_symbols(&items, 0),
                "resultCount": items.len(),
            }))
        }
        LspOperation::IncomingCalls => {
            let items = lsp_service::prepare_call_hierarchy(&uri, line, character).await?;
            if items.is_empty() {
                return Ok(json!({
                    "operation": "incomingCalls",
                    "result": "No call hierarchy items found at this position.",
                }));
            }
            let calls = lsp_service::incoming_calls(&items[0]).await?;
            Ok(json!({
                "operation": "incomingCalls",
                "filePath": file_path.display().to_string(),
                "result": format_symbols(&calls, 0),
                "resultCount": calls.len(),
            }))
        }
        LspOperation::OutgoingCalls => {
            let items = lsp_service::prepare_call_hierarchy(&uri, line, character).await?;
            if items.is_empty() {
                return Ok(json!({
                    "operation": "outgoingCalls",
                    "result": "No call hierarchy items found at this position.",
                }));
            }
            let calls = lsp_service::outgoing_calls(&items[0]).await?;
            Ok(json!({
                "operation": "outgoingCalls",
                "filePath": file_path.display().to_string(),
                "result": format_symbols(&calls, 0),
                "resultCount": calls.len(),
            }))
        }
    }
}

fn file_path_to_uri(path: &Path) -> String {
    // file:///path/to/file
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    format!(
        "file:///{}",
        abs.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_operation_from_str() {
        assert_eq!(
            LspOperation::from_str("goToDefinition"),
            Some(LspOperation::GoToDefinition)
        );
        assert_eq!(LspOperation::from_str("hover"), Some(LspOperation::Hover));
        assert_eq!(
            LspOperation::from_str("findReferences"),
            Some(LspOperation::FindReferences)
        );
        assert_eq!(LspOperation::from_str("invalid"), None);
    }

    #[test]
    fn test_lsp_operation_method() {
        assert_eq!(
            LspOperation::GoToDefinition.method(),
            "textDocument/definition"
        );
        assert_eq!(LspOperation::WorkspaceSymbol.method(), "workspace/symbol");
    }

    #[test]
    fn test_requires_position() {
        assert!(LspOperation::GoToDefinition.requires_position());
        assert!(LspOperation::Hover.requires_position());
        assert!(!LspOperation::WorkspaceSymbol.requires_position());
    }

    #[test]
    fn test_all_names() {
        let names = LspOperation::all_names();
        assert_eq!(names.len(), 9);
        assert!(names.contains(&"goToDefinition"));
        assert!(names.contains(&"outgoingCalls"));
    }

    #[test]
    fn test_format_locations_empty() {
        assert_eq!(format_locations(&[]), "No results found.");
    }

    #[test]
    fn test_format_locations() {
        let locs = vec![
            SourceLocation {
                file_path: "src/main.rs".into(),
                line: 10,
                character: 5,
                end_line: None,
                end_character: None,
            },
            SourceLocation {
                file_path: "src/main.rs".into(),
                line: 20,
                character: 1,
                end_line: None,
                end_character: None,
            },
        ];
        let text = format_locations(&locs);
        assert!(text.contains("src/main.rs"));
        assert!(text.contains("Line 10"));
        assert!(text.contains("Line 20"));
    }

    #[test]
    fn test_format_symbols() {
        let symbols = vec![SymbolInfo {
            name: "main".into(),
            kind: "function".into(),
            location: SourceLocation {
                file_path: "src/main.rs".into(),
                line: 1,
                character: 1,
                end_line: None,
                end_character: None,
            },
            children: vec![],
        }];
        let text = format_symbols(&symbols, 0);
        assert!(text.contains("function"));
        assert!(text.contains("`main`"));
    }

    #[test]
    fn test_format_hover_empty() {
        let hover = HoverInfo {
            contents: String::new(),
            range: None,
        };
        assert_eq!(format_hover(&hover), "No hover information available.");
    }

    #[tokio::test]
    async fn test_lsp_tool_basics() {
        let tool = LspTool;
        assert_eq!(tool.name(), "LSP");
        assert!(tool.is_read_only(&json!({})));
        assert!(tool.is_concurrency_safe(&json!({})));

        let schema = tool.input_json_schema();
        assert!(schema["properties"]["operation"].is_object());
        assert!(schema["properties"]["filePath"].is_object());
        assert!(schema["properties"]["line"].is_object());
    }

    #[test]
    fn test_user_facing_name() {
        let tool = LspTool;
        let input = json!({"operation": "hover", "filePath": "test.rs"});
        assert_eq!(tool.user_facing_name(Some(&input)), "LSP(hover)");
        assert_eq!(tool.user_facing_name(None), "LSP");
    }
}

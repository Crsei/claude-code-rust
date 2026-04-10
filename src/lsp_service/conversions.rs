//! Conversions from `lsp-types` protocol types to internal tool-layer types.
//!
//! All LSP positions are **0-based**; our [`SourceLocation`] is **1-based**.
//! Every conversion adds 1 to line and character values.

use anyhow::{bail, Context, Result};
use serde_json::Value;

use crate::tools::lsp::{HoverInfo, SourceLocation, SymbolInfo};

// ---------------------------------------------------------------------------
// URI helpers
// ---------------------------------------------------------------------------

/// Convert an `lsp_types::Uri` (file:// URI) to a file-system path string.
///
/// On Windows, strips the leading `/` from paths like `file:///C:/foo`.
pub fn uri_to_file_path(uri: &lsp_types::Uri) -> String {
    let s = uri.as_str();
    let path = if let Some(rest) = s.strip_prefix("file:///") {
        // Percent-decode is not performed here — LSP servers typically send
        // plain paths.  On Windows the URI looks like file:///C:/foo, so we
        // keep the drive letter intact.
        #[cfg(windows)]
        {
            rest.replace('/', "\\")
        }
        #[cfg(not(windows))]
        {
            format!("/{}", rest)
        }
    } else if let Some(rest) = s.strip_prefix("file://") {
        // file://host/path — rare, treat everything after `//` as path.
        rest.to_string()
    } else {
        // Fallback: return as-is.
        s.to_string()
    };
    path
}

/// Convert a file-system path string to an `lsp_types::Uri`.
///
/// Handles Windows backslashes by converting them to forward slashes.
pub fn file_path_to_uri(path: &str) -> Result<lsp_types::Uri> {
    let normalized = path.replace('\\', "/");
    // Ensure an absolute path starts with `/` (Unix) or a drive letter (Windows).
    let uri_string = if normalized.starts_with('/') {
        format!("file://{}", normalized)
    } else {
        // Windows absolute path like `C:/foo` → `file:///C:/foo`
        format!("file:///{}", normalized)
    };
    uri_string
        .parse::<lsp_types::Uri>()
        .map_err(|e| anyhow::anyhow!("failed to parse URI '{}': {}", uri_string, e))
}

// ---------------------------------------------------------------------------
// Location conversions
// ---------------------------------------------------------------------------

/// Convert an `lsp_types::Location` to a [`SourceLocation`] (0-based → 1-based).
pub fn location_to_source(loc: &lsp_types::Location) -> SourceLocation {
    SourceLocation {
        file_path: uri_to_file_path(&loc.uri),
        line: loc.range.start.line + 1,
        character: loc.range.start.character + 1,
        end_line: Some(loc.range.end.line + 1),
        end_character: Some(loc.range.end.character + 1),
    }
}

/// Convert an `lsp_types::LocationLink` to a [`SourceLocation`] (0-based → 1-based).
///
/// Uses `target_uri` and `target_selection_range` for the location.
pub fn location_link_to_source(link: &lsp_types::LocationLink) -> SourceLocation {
    SourceLocation {
        file_path: uri_to_file_path(&link.target_uri),
        line: link.target_selection_range.start.line + 1,
        character: link.target_selection_range.start.character + 1,
        end_line: Some(link.target_selection_range.end.line + 1),
        end_character: Some(link.target_selection_range.end.character + 1),
    }
}

/// Parse a JSON response that may be `null`, a single `Location`, a
/// `Location[]`, or a `LocationLink[]`.
///
/// This corresponds to the `GotoDefinitionResponse` union and similar
/// response types in the LSP specification.
pub fn parse_location_response(value: Value) -> Result<Vec<SourceLocation>> {
    if value.is_null() {
        return Ok(vec![]);
    }

    // Try GotoDefinitionResponse (Scalar | Array | Link)
    if let Ok(resp) = serde_json::from_value::<lsp_types::GotoDefinitionResponse>(value.clone()) {
        return Ok(match resp {
            lsp_types::GotoDefinitionResponse::Scalar(loc) => {
                vec![location_to_source(&loc)]
            }
            lsp_types::GotoDefinitionResponse::Array(locs) => {
                locs.iter().map(location_to_source).collect()
            }
            lsp_types::GotoDefinitionResponse::Link(links) => {
                links.iter().map(location_link_to_source).collect()
            }
        });
    }

    // Try single Location
    if let Ok(loc) = serde_json::from_value::<lsp_types::Location>(value.clone()) {
        return Ok(vec![location_to_source(&loc)]);
    }

    // Try Location[]
    if let Ok(locs) = serde_json::from_value::<Vec<lsp_types::Location>>(value.clone()) {
        return Ok(locs.iter().map(location_to_source).collect());
    }

    // Try LocationLink[]
    if let Ok(links) = serde_json::from_value::<Vec<lsp_types::LocationLink>>(value.clone()) {
        return Ok(links.iter().map(location_link_to_source).collect());
    }

    bail!("Unable to parse location response")
}

// ---------------------------------------------------------------------------
// Hover
// ---------------------------------------------------------------------------

/// Extract a plain string from `MarkedString`.
fn marked_string_to_text(ms: &lsp_types::MarkedString) -> String {
    match ms {
        lsp_types::MarkedString::String(s) => s.clone(),
        lsp_types::MarkedString::LanguageString(ls) => {
            format!("```{}\n{}\n```", ls.language, ls.value)
        }
    }
}

/// Convert an `lsp_types::Range` to an optional [`SourceLocation`] (no file path).
fn range_to_source_location(range: &lsp_types::Range) -> SourceLocation {
    SourceLocation {
        file_path: String::new(),
        line: range.start.line + 1,
        character: range.start.character + 1,
        end_line: Some(range.end.line + 1),
        end_character: Some(range.end.character + 1),
    }
}

/// Parse a hover response JSON value into [`HoverInfo`].
///
/// Handles `null`, `Hover { contents: MarkupContent | MarkedString | MarkedString[], range }`.
pub fn parse_hover_response(value: Value) -> Result<HoverInfo> {
    if value.is_null() {
        return Ok(HoverInfo {
            contents: String::new(),
            range: None,
        });
    }

    let hover: lsp_types::Hover =
        serde_json::from_value(value).context("Failed to parse Hover response")?;

    let contents = match &hover.contents {
        lsp_types::HoverContents::Scalar(ms) => marked_string_to_text(ms),
        lsp_types::HoverContents::Array(arr) => arr
            .iter()
            .map(marked_string_to_text)
            .collect::<Vec<_>>()
            .join("\n\n"),
        lsp_types::HoverContents::Markup(mc) => mc.value.clone(),
    };

    let range = hover.range.as_ref().map(range_to_source_location);

    Ok(HoverInfo { contents, range })
}

// ---------------------------------------------------------------------------
// Symbols
// ---------------------------------------------------------------------------

/// Convert an `lsp_types::SymbolKind` to a human-readable string.
///
/// Covers all 26 standard symbol kinds defined in LSP 3.17.
pub fn symbol_kind_str(kind: lsp_types::SymbolKind) -> &'static str {
    match kind {
        lsp_types::SymbolKind::FILE => "File",
        lsp_types::SymbolKind::MODULE => "Module",
        lsp_types::SymbolKind::NAMESPACE => "Namespace",
        lsp_types::SymbolKind::PACKAGE => "Package",
        lsp_types::SymbolKind::CLASS => "Class",
        lsp_types::SymbolKind::METHOD => "Method",
        lsp_types::SymbolKind::PROPERTY => "Property",
        lsp_types::SymbolKind::FIELD => "Field",
        lsp_types::SymbolKind::CONSTRUCTOR => "Constructor",
        lsp_types::SymbolKind::ENUM => "Enum",
        lsp_types::SymbolKind::INTERFACE => "Interface",
        lsp_types::SymbolKind::FUNCTION => "Function",
        lsp_types::SymbolKind::VARIABLE => "Variable",
        lsp_types::SymbolKind::CONSTANT => "Constant",
        lsp_types::SymbolKind::STRING => "String",
        lsp_types::SymbolKind::NUMBER => "Number",
        lsp_types::SymbolKind::BOOLEAN => "Boolean",
        lsp_types::SymbolKind::ARRAY => "Array",
        lsp_types::SymbolKind::OBJECT => "Object",
        lsp_types::SymbolKind::KEY => "Key",
        lsp_types::SymbolKind::NULL => "Null",
        lsp_types::SymbolKind::ENUM_MEMBER => "EnumMember",
        lsp_types::SymbolKind::STRUCT => "Struct",
        lsp_types::SymbolKind::EVENT => "Event",
        lsp_types::SymbolKind::OPERATOR => "Operator",
        lsp_types::SymbolKind::TYPE_PARAMETER => "TypeParameter",
        _ => "Unknown",
    }
}

/// Recursively convert an `lsp_types::DocumentSymbol` (hierarchical) into [`SymbolInfo`].
fn document_symbol_to_info(sym: &lsp_types::DocumentSymbol, file_path: &str) -> SymbolInfo {
    let children = sym
        .children
        .as_ref()
        .map(|kids| {
            kids.iter()
                .map(|c| document_symbol_to_info(c, file_path))
                .collect()
        })
        .unwrap_or_default();

    SymbolInfo {
        name: sym.name.clone(),
        kind: symbol_kind_str(sym.kind).to_string(),
        location: SourceLocation {
            file_path: file_path.to_string(),
            line: sym.selection_range.start.line + 1,
            character: sym.selection_range.start.character + 1,
            end_line: Some(sym.selection_range.end.line + 1),
            end_character: Some(sym.selection_range.end.character + 1),
        },
        children,
    }
}

/// Convert an `lsp_types::SymbolInformation` (flat) into [`SymbolInfo`].
fn symbol_information_to_info(sym: &lsp_types::SymbolInformation) -> SymbolInfo {
    SymbolInfo {
        name: sym.name.clone(),
        kind: symbol_kind_str(sym.kind).to_string(),
        location: location_to_source(&sym.location),
        children: vec![],
    }
}

/// Parse a `textDocument/documentSymbol` response.
///
/// Handles both `DocumentSymbol[]` (hierarchical, with children) and
/// `SymbolInformation[]` (flat, no children).
pub fn parse_document_symbols_response(value: Value) -> Result<Vec<SymbolInfo>> {
    if value.is_null() {
        return Ok(vec![]);
    }

    let resp: lsp_types::DocumentSymbolResponse = serde_json::from_value(value)
        .context("Failed to parse document symbols response")?;

    Ok(match resp {
        lsp_types::DocumentSymbolResponse::Nested(symbols) => {
            // DocumentSymbol doesn't carry a URI — use empty string as placeholder.
            // Callers can patch this with the known document URI.
            symbols
                .iter()
                .map(|s| document_symbol_to_info(s, ""))
                .collect()
        }
        lsp_types::DocumentSymbolResponse::Flat(infos) => {
            infos.iter().map(symbol_information_to_info).collect()
        }
    })
}

/// Parse a `workspace/symbol` response.
///
/// Handles `SymbolInformation[]` and `WorkspaceSymbol[]` (3.17+).
pub fn parse_workspace_symbols_response(value: Value) -> Result<Vec<SymbolInfo>> {
    if value.is_null() {
        return Ok(vec![]);
    }

    // Try WorkspaceSymbolResponse first (handles both variants).
    if let Ok(resp) = serde_json::from_value::<lsp_types::WorkspaceSymbolResponse>(value.clone()) {
        return Ok(match resp {
            lsp_types::WorkspaceSymbolResponse::Flat(infos) => {
                infos.iter().map(symbol_information_to_info).collect()
            }
            lsp_types::WorkspaceSymbolResponse::Nested(ws_symbols) => ws_symbols
                .iter()
                .map(|ws| {
                    let file_path = match &ws.location {
                        lsp_types::OneOf::Left(loc) => uri_to_file_path(&loc.uri),
                        lsp_types::OneOf::Right(wl) => uri_to_file_path(&wl.uri),
                    };
                    let location = match &ws.location {
                        lsp_types::OneOf::Left(loc) => location_to_source(loc),
                        lsp_types::OneOf::Right(_wl) => SourceLocation {
                            file_path,
                            line: 1,
                            character: 1,
                            end_line: None,
                            end_character: None,
                        },
                    };
                    SymbolInfo {
                        name: ws.name.clone(),
                        kind: symbol_kind_str(ws.kind).to_string(),
                        location,
                        children: vec![],
                    }
                })
                .collect(),
        });
    }

    bail!("Failed to parse workspace symbols response")
}

// ---------------------------------------------------------------------------
// Call hierarchy
// ---------------------------------------------------------------------------

/// Convert a `CallHierarchyItem` into a [`SymbolInfo`].
pub fn call_hierarchy_item_to_info(item: &lsp_types::CallHierarchyItem) -> SymbolInfo {
    SymbolInfo {
        name: item.name.clone(),
        kind: symbol_kind_str(item.kind).to_string(),
        location: SourceLocation {
            file_path: uri_to_file_path(&item.uri),
            line: item.selection_range.start.line + 1,
            character: item.selection_range.start.character + 1,
            end_line: Some(item.selection_range.end.line + 1),
            end_character: Some(item.selection_range.end.character + 1),
        },
        children: vec![],
    }
}

/// Parse a `textDocument/prepareCallHierarchy` response into a list of [`SymbolInfo`].
pub fn parse_call_hierarchy_items(value: Value) -> Result<Vec<SymbolInfo>> {
    if value.is_null() {
        return Ok(vec![]);
    }

    let items: Vec<lsp_types::CallHierarchyItem> =
        serde_json::from_value(value).context("Failed to parse CallHierarchyItem[]")?;

    Ok(items.iter().map(call_hierarchy_item_to_info).collect())
}

/// Parse a `callHierarchy/incomingCalls` response.
///
/// Extracts the `.from` field of each `CallHierarchyIncomingCall`.
pub fn parse_incoming_calls(value: Value) -> Result<Vec<SymbolInfo>> {
    if value.is_null() {
        return Ok(vec![]);
    }

    let calls: Vec<lsp_types::CallHierarchyIncomingCall> =
        serde_json::from_value(value).context("Failed to parse CallHierarchyIncomingCall[]")?;

    Ok(calls
        .iter()
        .map(|c| call_hierarchy_item_to_info(&c.from))
        .collect())
}

/// Parse a `callHierarchy/outgoingCalls` response.
///
/// Extracts the `.to` field of each `CallHierarchyOutgoingCall`.
pub fn parse_outgoing_calls(value: Value) -> Result<Vec<SymbolInfo>> {
    if value.is_null() {
        return Ok(vec![]);
    }

    let calls: Vec<lsp_types::CallHierarchyOutgoingCall> =
        serde_json::from_value(value).context("Failed to parse CallHierarchyOutgoingCall[]")?;

    Ok(calls
        .iter()
        .map(|c| call_hierarchy_item_to_info(&c.to))
        .collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- URI helpers --------------------------------------------------------

    #[test]
    fn test_uri_to_file_path_unix() {
        let uri: lsp_types::Uri = "file:///home/user/project/main.rs".parse().unwrap();
        let path = uri_to_file_path(&uri);
        #[cfg(windows)]
        assert_eq!(path, "home\\user\\project\\main.rs");
        #[cfg(not(windows))]
        assert_eq!(path, "/home/user/project/main.rs");
    }

    #[cfg(windows)]
    #[test]
    fn test_uri_to_file_path_windows() {
        let uri: lsp_types::Uri = "file:///C:/Users/dev/project/main.rs".parse().unwrap();
        let path = uri_to_file_path(&uri);
        assert_eq!(path, "C:\\Users\\dev\\project\\main.rs");
    }

    #[test]
    fn test_file_path_to_uri_unix() {
        let uri = file_path_to_uri("/home/user/main.rs").unwrap();
        assert_eq!(uri.as_str(), "file:///home/user/main.rs");
    }

    #[test]
    fn test_file_path_to_uri_windows_backslash() {
        let uri = file_path_to_uri("C:\\Users\\dev\\main.rs").unwrap();
        assert_eq!(uri.as_str(), "file:///C:/Users/dev/main.rs");
    }

    #[test]
    fn test_file_path_to_uri_windows_forward_slash() {
        let uri = file_path_to_uri("D:/projects/src/lib.rs").unwrap();
        assert_eq!(uri.as_str(), "file:///D:/projects/src/lib.rs");
    }

    #[test]
    fn test_uri_roundtrip_unix() {
        let original = "/tmp/test/file.rs";
        let uri = file_path_to_uri(original).unwrap();
        let back = uri_to_file_path(&uri);
        #[cfg(not(windows))]
        assert_eq!(back, original);
    }

    // -- Location conversion -----------------------------------------------

    #[test]
    fn test_location_to_source_zero_to_one_based() {
        let uri: lsp_types::Uri = "file:///src/main.rs".parse().unwrap();
        let loc = lsp_types::Location {
            uri,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 5,
                },
            },
        };
        let src = location_to_source(&loc);
        assert_eq!(src.line, 1, "0-based line 0 should become 1");
        assert_eq!(src.character, 1, "0-based char 0 should become 1");
        assert_eq!(src.end_line, Some(1));
        assert_eq!(src.end_character, Some(6));
    }

    #[test]
    fn test_location_to_source_nonzero() {
        let uri: lsp_types::Uri = "file:///src/lib.rs".parse().unwrap();
        let loc = lsp_types::Location {
            uri,
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 9,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 15,
                    character: 0,
                },
            },
        };
        let src = location_to_source(&loc);
        assert_eq!(src.line, 10);
        assert_eq!(src.character, 5);
        assert_eq!(src.end_line, Some(16));
        assert_eq!(src.end_character, Some(1));
    }

    #[test]
    fn test_location_link_to_source() {
        let link = lsp_types::LocationLink {
            origin_selection_range: None,
            target_uri: "file:///src/types.rs".parse().unwrap(),
            target_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 50,
                    character: 0,
                },
            },
            target_selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 3,
                    character: 10,
                },
                end: lsp_types::Position {
                    line: 3,
                    character: 20,
                },
            },
        };
        let src = location_link_to_source(&link);
        assert_eq!(src.line, 4);
        assert_eq!(src.character, 11);
        assert_eq!(src.end_line, Some(4));
        assert_eq!(src.end_character, Some(21));
    }

    // -- parse_location_response -------------------------------------------

    #[test]
    fn test_parse_location_response_null() {
        let result = parse_location_response(Value::Null).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_location_response_single() {
        let json = json!({
            "uri": "file:///src/main.rs",
            "range": {
                "start": {"line": 5, "character": 0},
                "end": {"line": 5, "character": 10}
            }
        });
        let result = parse_location_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].line, 6);
        assert_eq!(result[0].character, 1);
    }

    #[test]
    fn test_parse_location_response_array() {
        let json = json!([
            {
                "uri": "file:///a.rs",
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 5}
                }
            },
            {
                "uri": "file:///b.rs",
                "range": {
                    "start": {"line": 10, "character": 3},
                    "end": {"line": 10, "character": 8}
                }
            }
        ]);
        let result = parse_location_response(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].line, 1);
        assert_eq!(result[1].line, 11);
        assert_eq!(result[1].character, 4);
    }

    #[test]
    fn test_parse_location_response_link_array() {
        let json = json!([
            {
                "targetUri": "file:///src/lib.rs",
                "targetRange": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 20, "character": 0}
                },
                "targetSelectionRange": {
                    "start": {"line": 5, "character": 4},
                    "end": {"line": 5, "character": 15}
                }
            }
        ]);
        let result = parse_location_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].line, 6);
        assert_eq!(result[0].character, 5);
    }

    // -- Hover -------------------------------------------------------------

    #[test]
    fn test_parse_hover_response_null() {
        let hover = parse_hover_response(Value::Null).unwrap();
        assert!(hover.contents.is_empty());
        assert!(hover.range.is_none());
    }

    #[test]
    fn test_parse_hover_response_markup() {
        let json = json!({
            "contents": {
                "kind": "markdown",
                "value": "```rust\nfn main()\n```"
            }
        });
        let hover = parse_hover_response(json).unwrap();
        assert_eq!(hover.contents, "```rust\nfn main()\n```");
        assert!(hover.range.is_none());
    }

    #[test]
    fn test_parse_hover_response_scalar_string() {
        let json = json!({
            "contents": "Hello hover"
        });
        let hover = parse_hover_response(json).unwrap();
        assert_eq!(hover.contents, "Hello hover");
    }

    #[test]
    fn test_parse_hover_response_language_string() {
        let json = json!({
            "contents": {"language": "rust", "value": "fn foo()"}
        });
        let hover = parse_hover_response(json).unwrap();
        assert!(hover.contents.contains("```rust"));
        assert!(hover.contents.contains("fn foo()"));
    }

    #[test]
    fn test_parse_hover_response_array() {
        let json = json!({
            "contents": [
                "Some doc",
                {"language": "go", "value": "func Bar()"}
            ]
        });
        let hover = parse_hover_response(json).unwrap();
        assert!(hover.contents.contains("Some doc"));
        assert!(hover.contents.contains("```go"));
        assert!(hover.contents.contains("func Bar()"));
    }

    #[test]
    fn test_parse_hover_with_range() {
        let json = json!({
            "contents": {"kind": "plaintext", "value": "int x"},
            "range": {
                "start": {"line": 2, "character": 4},
                "end": {"line": 2, "character": 9}
            }
        });
        let hover = parse_hover_response(json).unwrap();
        assert_eq!(hover.contents, "int x");
        let r = hover.range.unwrap();
        assert_eq!(r.line, 3);
        assert_eq!(r.character, 5);
    }

    // -- Symbol kind -------------------------------------------------------

    #[test]
    fn test_symbol_kind_str_all_variants() {
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::FILE), "File");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::MODULE), "Module");
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::NAMESPACE),
            "Namespace"
        );
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::PACKAGE), "Package");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::CLASS), "Class");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::METHOD), "Method");
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::PROPERTY),
            "Property"
        );
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::FIELD), "Field");
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::CONSTRUCTOR),
            "Constructor"
        );
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::ENUM), "Enum");
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::INTERFACE),
            "Interface"
        );
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::FUNCTION),
            "Function"
        );
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::VARIABLE),
            "Variable"
        );
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::CONSTANT),
            "Constant"
        );
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::STRING), "String");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::NUMBER), "Number");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::BOOLEAN), "Boolean");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::ARRAY), "Array");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::OBJECT), "Object");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::KEY), "Key");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::NULL), "Null");
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::ENUM_MEMBER),
            "EnumMember"
        );
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::STRUCT), "Struct");
        assert_eq!(symbol_kind_str(lsp_types::SymbolKind::EVENT), "Event");
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::OPERATOR),
            "Operator"
        );
        assert_eq!(
            symbol_kind_str(lsp_types::SymbolKind::TYPE_PARAMETER),
            "TypeParameter"
        );
    }

    // -- Document symbols --------------------------------------------------

    #[test]
    fn test_parse_document_symbols_null() {
        let result = parse_document_symbols_response(Value::Null).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_document_symbols_flat() {
        let json = json!([
            {
                "name": "MyClass",
                "kind": 5,
                "location": {
                    "uri": "file:///src/lib.rs",
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 10, "character": 0}
                    }
                }
            }
        ]);
        let result = parse_document_symbols_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "MyClass");
        assert_eq!(result[0].kind, "Class");
        assert_eq!(result[0].location.line, 1);
    }

    #[test]
    #[allow(deprecated)] // DocumentSymbol.deprecated field
    fn test_parse_document_symbols_nested_with_children() {
        let json = json!([
            {
                "name": "MyStruct",
                "kind": 23,
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 20, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 4},
                    "end": {"line": 0, "character": 12}
                },
                "children": [
                    {
                        "name": "field_a",
                        "kind": 8,
                        "range": {
                            "start": {"line": 1, "character": 4},
                            "end": {"line": 1, "character": 20}
                        },
                        "selectionRange": {
                            "start": {"line": 1, "character": 4},
                            "end": {"line": 1, "character": 11}
                        }
                    }
                ]
            }
        ]);
        let result = parse_document_symbols_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "MyStruct");
        assert_eq!(result[0].kind, "Struct");
        assert_eq!(result[0].location.line, 1);
        assert_eq!(result[0].location.character, 5);
        assert_eq!(result[0].children.len(), 1);
        assert_eq!(result[0].children[0].name, "field_a");
        assert_eq!(result[0].children[0].kind, "Field");
        assert_eq!(result[0].children[0].location.line, 2);
    }

    // -- Workspace symbols -------------------------------------------------

    #[test]
    fn test_parse_workspace_symbols_null() {
        let result = parse_workspace_symbols_response(Value::Null).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_workspace_symbols_flat() {
        let json = json!([
            {
                "name": "global_var",
                "kind": 13,
                "location": {
                    "uri": "file:///src/globals.rs",
                    "range": {
                        "start": {"line": 4, "character": 0},
                        "end": {"line": 4, "character": 20}
                    }
                }
            }
        ]);
        let result = parse_workspace_symbols_response(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "global_var");
        assert_eq!(result[0].kind, "Variable");
        assert_eq!(result[0].location.line, 5);
    }

    // -- Call hierarchy ----------------------------------------------------

    #[test]
    fn test_call_hierarchy_item_to_info() {
        let item = lsp_types::CallHierarchyItem {
            name: "do_work".into(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: "file:///src/work.rs".parse().unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 25,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 10,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 10,
                    character: 11,
                },
            },
            data: None,
        };
        let info = call_hierarchy_item_to_info(&item);
        assert_eq!(info.name, "do_work");
        assert_eq!(info.kind, "Function");
        assert_eq!(info.location.line, 11);
        assert_eq!(info.location.character, 5);
    }

    #[test]
    fn test_parse_call_hierarchy_items_null() {
        let result = parse_call_hierarchy_items(Value::Null).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_call_hierarchy_items() {
        let json = json!([
            {
                "name": "init",
                "kind": 12,
                "uri": "file:///src/app.rs",
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 5, "character": 1}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 3},
                    "end": {"line": 0, "character": 7}
                }
            }
        ]);
        let result = parse_call_hierarchy_items(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "init");
        assert_eq!(result[0].kind, "Function");
        assert_eq!(result[0].location.line, 1);
        assert_eq!(result[0].location.character, 4);
    }

    #[test]
    fn test_parse_incoming_calls() {
        let json = json!([
            {
                "from": {
                    "name": "caller_fn",
                    "kind": 12,
                    "uri": "file:///src/caller.rs",
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 10, "character": 1}
                    },
                    "selectionRange": {
                        "start": {"line": 0, "character": 3},
                        "end": {"line": 0, "character": 12}
                    }
                },
                "fromRanges": [
                    {
                        "start": {"line": 5, "character": 4},
                        "end": {"line": 5, "character": 15}
                    }
                ]
            }
        ]);
        let result = parse_incoming_calls(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "caller_fn");
        assert_eq!(result[0].kind, "Function");
    }

    #[test]
    fn test_parse_outgoing_calls() {
        let json = json!([
            {
                "to": {
                    "name": "callee_fn",
                    "kind": 12,
                    "uri": "file:///src/callee.rs",
                    "range": {
                        "start": {"line": 20, "character": 0},
                        "end": {"line": 30, "character": 1}
                    },
                    "selectionRange": {
                        "start": {"line": 20, "character": 3},
                        "end": {"line": 20, "character": 12}
                    }
                },
                "fromRanges": [
                    {
                        "start": {"line": 7, "character": 8},
                        "end": {"line": 7, "character": 17}
                    }
                ]
            }
        ]);
        let result = parse_outgoing_calls(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "callee_fn");
        assert_eq!(result[0].kind, "Function");
        assert_eq!(result[0].location.line, 21);
    }

    #[test]
    fn test_parse_incoming_calls_null() {
        let result = parse_incoming_calls(Value::Null).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_outgoing_calls_null() {
        let result = parse_outgoing_calls(Value::Null).unwrap();
        assert!(result.is_empty());
    }
}

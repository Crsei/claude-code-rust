//! MCP tool integration — wraps MCP tools as local Tool trait objects.
//!
//! Each MCP tool is wrapped in `McpToolWrapper` which delegates `call()` to
//! the real `McpClient` for JSON-RPC `tools/call` execution. The wrapper
//! holds a reference to the manager so it can find the right client at
//! call time.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tracing::debug;

use crate::types::message::{AssistantMessage, ContentBlock, ImageSource, ToolResultContent};
use crate::types::tool::*;

use super::manager::McpManager;
use super::{McpToolDef, ToolCallContent};

// ---------------------------------------------------------------------------
// McpToolWrapper
// ---------------------------------------------------------------------------

/// Wraps an MCP tool definition as a local `Tool` trait object.
///
/// When `call()` is invoked, the wrapper looks up the owning `McpClient`
/// via the shared `McpManager` and sends a `tools/call` JSON-RPC request.
pub struct McpToolWrapper {
    /// The tool definition from the MCP server.
    pub def: McpToolDef,
    /// Server name that provides this tool.
    pub server_name: String,
    /// Shared manager for accessing the MCP client at call time.
    pub manager: Arc<Mutex<McpManager>>,
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        // MCP tools are namespaced as mcp__{server}__{tool}
        // But we store the full name in def.name for simplicity
        &self.def.name
    }

    async fn description(&self, _input: &Value) -> String {
        self.def.description.clone()
    }

    fn input_json_schema(&self) -> Value {
        self.def.input_schema.clone()
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        // MCP tools share a single stdin/stdout channel per server,
        // but the client handles multiplexing via request IDs.
        true
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        false // We don't know — assume MCP tools may have side effects
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        debug!(
            server = %self.server_name,
            tool = %self.def.name,
            "MCP tool call"
        );

        let manager = self.manager.lock().await;

        let client = manager.clients.get(&self.server_name).ok_or_else(|| {
            anyhow::anyhow!(
                "MCP server '{}' not connected (tool '{}')",
                self.server_name,
                self.def.name
            )
        })?;

        let result = client.call_tool(&self.def.name, input).await?;

        // Build display-only text (for UI/logs — images become "[Image: mime]")
        let display_text = format_tool_call_result(&result.content, result.is_error);

        if result.is_error {
            Ok(ToolResult {
                data: json!(format!("MCP tool error: {}", display_text)),
                new_messages: vec![],
                ..Default::default()
            })
        } else {
            // Check if content contains any non-text blocks (images, resources with blobs)
            let has_multimodal = result.content.iter().any(|b| matches!(b, ToolCallContent::Image { .. }));

            if has_multimodal {
                let model_blocks = convert_mcp_to_content_blocks(&result.content);
                Ok(ToolResult::with_content(
                    json!(display_text),
                    ToolResultContent::Blocks(model_blocks),
                    display_text,
                ))
            } else {
                Ok(ToolResult {
                    data: json!(display_text),
                    new_messages: vec![],
                    ..Default::default()
                })
            }
        }
    }

    async fn prompt(&self) -> String {
        format!(
            "MCP tool '{}' from server '{}': {}",
            self.def.name, self.server_name, self.def.description
        )
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        format!("mcp__{}__{}", self.server_name, self.def.name)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert MCP tool call content blocks to Anthropic API ContentBlocks.
///
/// Text blocks become `ContentBlock::Text`, image blocks become `ContentBlock::Image`
/// with base64 source. Resource blocks with text become `ContentBlock::Text`.
fn convert_mcp_to_content_blocks(content: &[ToolCallContent]) -> Vec<ContentBlock> {
    let mut blocks = Vec::new();

    for item in content {
        match item {
            ToolCallContent::Text { text } => {
                blocks.push(ContentBlock::Text { text: text.clone() });
            }
            ToolCallContent::Image { data, mime_type } => {
                blocks.push(ContentBlock::Image {
                    source: ImageSource {
                        source_type: "base64".to_string(),
                        media_type: mime_type.clone(),
                        data: data.clone(),
                    },
                });
            }
            ToolCallContent::Resource { resource } => {
                // Resources with text content become text blocks
                if let Some(ref text) = resource.text {
                    blocks.push(ContentBlock::Text {
                        text: format!("[Resource: {}]\n{}", resource.uri, text),
                    });
                } else if let Some(ref blob) = resource.blob {
                    // Binary resources with a recognized image MIME type become image blocks
                    let mime = resource.mime_type.as_deref().unwrap_or("application/octet-stream");
                    if mime.starts_with("image/") {
                        blocks.push(ContentBlock::Image {
                            source: ImageSource {
                                source_type: "base64".to_string(),
                                media_type: mime.to_string(),
                                data: blob.clone(),
                            },
                        });
                    } else {
                        blocks.push(ContentBlock::Text {
                            text: format!("[Resource: {} (binary)]", resource.uri),
                        });
                    }
                } else {
                    blocks.push(ContentBlock::Text {
                        text: format!("[Resource: {}]", resource.uri),
                    });
                }
            }
        }
    }

    blocks
}

/// Format MCP tool call content blocks into a single string.
fn format_tool_call_result(content: &[ToolCallContent], is_error: bool) -> String {
    let mut parts: Vec<String> = Vec::new();

    for block in content {
        match block {
            ToolCallContent::Text { text } => {
                parts.push(text.clone());
            }
            ToolCallContent::Image { mime_type, .. } => {
                parts.push(format!("[Image: {}]", mime_type));
            }
            ToolCallContent::Resource { resource } => {
                if let Some(ref text) = resource.text {
                    parts.push(format!("[Resource: {}]\n{}", resource.uri, text));
                } else if resource.blob.is_some() {
                    parts.push(format!("[Resource: {} (binary)]", resource.uri));
                } else {
                    parts.push(format!("[Resource: {}]", resource.uri));
                }
            }
        }
    }

    if parts.is_empty() {
        if is_error {
            "(MCP tool returned error with no content)".to_string()
        } else {
            "(MCP tool returned no content)".to_string()
        }
    } else {
        parts.join("\n")
    }
}

/// Convert all MCP tool definitions into `Tool` trait objects.
///
/// Each tool gets a shared reference to the `McpManager` so it can
/// delegate `call()` at runtime.
pub fn mcp_tools_to_tools(
    defs: Vec<McpToolDef>,
    manager: Arc<Mutex<McpManager>>,
) -> Vec<Arc<dyn Tool>> {
    defs.into_iter()
        .map(|def| {
            let server_name = def.server_name.clone();
            Arc::new(McpToolWrapper {
                def,
                server_name,
                manager: manager.clone(),
            }) as Arc<dyn Tool>
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tool_call_result_text() {
        let content = vec![ToolCallContent::Text {
            text: "hello world".to_string(),
        }];
        let result = format_tool_call_result(&content, false);
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_format_tool_call_result_multiple_blocks() {
        let content = vec![
            ToolCallContent::Text {
                text: "line 1".to_string(),
            },
            ToolCallContent::Text {
                text: "line 2".to_string(),
            },
        ];
        let result = format_tool_call_result(&content, false);
        assert_eq!(result, "line 1\nline 2");
    }

    #[test]
    fn test_format_tool_call_result_image() {
        let content = vec![ToolCallContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        }];
        let result = format_tool_call_result(&content, false);
        assert_eq!(result, "[Image: image/png]");
    }

    #[test]
    fn test_format_tool_call_result_resource_text() {
        let content = vec![ToolCallContent::Resource {
            resource: super::super::McpResourceContent {
                uri: "file:///tmp/test.txt".to_string(),
                mime_type: Some("text/plain".to_string()),
                text: Some("file contents".to_string()),
                blob: None,
            },
        }];
        let result = format_tool_call_result(&content, false);
        assert!(result.contains("file:///tmp/test.txt"));
        assert!(result.contains("file contents"));
    }

    #[test]
    fn test_format_tool_call_result_empty() {
        let result = format_tool_call_result(&[], false);
        assert_eq!(result, "(MCP tool returned no content)");

        let result_err = format_tool_call_result(&[], true);
        assert!(result_err.contains("error"));
    }

    #[test]
    fn test_mcp_tool_wrapper_user_facing_name() {
        let manager = Arc::new(Mutex::new(McpManager::new()));
        let wrapper = McpToolWrapper {
            def: McpToolDef {
                name: "read_file".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({}),
                server_name: "filesystem".to_string(),
            },
            server_name: "filesystem".to_string(),
            manager,
        };

        assert_eq!(wrapper.user_facing_name(None), "mcp__filesystem__read_file");
    }

    #[test]
    fn test_mcp_tools_to_tools() {
        let manager = Arc::new(Mutex::new(McpManager::new()));
        let defs = vec![
            McpToolDef {
                name: "tool_a".to_string(),
                description: "Tool A".to_string(),
                input_schema: json!({"type": "object"}),
                server_name: "server1".to_string(),
            },
            McpToolDef {
                name: "tool_b".to_string(),
                description: "Tool B".to_string(),
                input_schema: json!({"type": "object"}),
                server_name: "server1".to_string(),
            },
        ];

        let tools = mcp_tools_to_tools(defs, manager);
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name(), "tool_a");
        assert_eq!(tools[1].name(), "tool_b");
    }

    #[test]
    fn test_convert_mcp_image_to_content_block() {
        let content = vec![ToolCallContent::Image {
            data: "iVBORw0KGgo=".to_string(),
            mime_type: "image/png".to_string(),
        }];
        let blocks = convert_mcp_to_content_blocks(&content);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Image { source } => {
                assert_eq!(source.source_type, "base64");
                assert_eq!(source.media_type, "image/png");
                assert_eq!(source.data, "iVBORw0KGgo=");
            }
            other => panic!("expected Image block, got {:?}", other),
        }
    }

    #[test]
    fn test_convert_mcp_mixed_text_and_image() {
        let content = vec![
            ToolCallContent::Text {
                text: "Screenshot taken".to_string(),
            },
            ToolCallContent::Image {
                data: "base64png".to_string(),
                mime_type: "image/png".to_string(),
            },
        ];
        let blocks = convert_mcp_to_content_blocks(&content);
        assert_eq!(blocks.len(), 2);
        assert!(matches!(&blocks[0], ContentBlock::Text { text } if text == "Screenshot taken"));
        assert!(matches!(&blocks[1], ContentBlock::Image { .. }));
    }

    #[test]
    fn test_convert_mcp_resource_blob_image() {
        let content = vec![ToolCallContent::Resource {
            resource: super::super::McpResourceContent {
                uri: "screenshot://latest".to_string(),
                mime_type: Some("image/jpeg".to_string()),
                text: None,
                blob: Some("base64jpeg".to_string()),
            },
        }];
        let blocks = convert_mcp_to_content_blocks(&content);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Image { source } => {
                assert_eq!(source.media_type, "image/jpeg");
                assert_eq!(source.data, "base64jpeg");
            }
            other => panic!("expected Image block, got {:?}", other),
        }
    }
}

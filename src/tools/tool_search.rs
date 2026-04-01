//! ToolSearch tool — searches available tools by keyword/name.
//!
//! Corresponds to TypeScript: tools/ToolSearchTool/
//!
//! Fetches full schema definitions for tools whose names or descriptions
//! match the query. Supports exact selection via "select:Name1,Name2"
//! syntax and keyword-based fuzzy search.

#![allow(unused)]

use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::debug;

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

/// ToolSearch tool — find available tools by keyword.
pub struct ToolSearchTool;

#[derive(Deserialize)]
struct ToolSearchInput {
    /// Search query. Use "select:Name1,Name2" for exact selection,
    /// or keywords to search by name/description.
    query: String,
    /// Maximum number of results to return.
    #[serde(default = "default_max_results")]
    max_results: usize,
}

fn default_max_results() -> usize {
    5
}

/// Compute a simple relevance score for a tool against a query.
///
/// Returns 0 if no match, higher values for better matches.
fn score_tool(tool: &dyn Tool, query_lower: &str, keywords: &[&str]) -> usize {
    let name_lower = tool.name().to_lowercase();
    let mut score = 0;

    // Exact name match
    if name_lower == query_lower {
        return 1000;
    }

    // Name contains query
    if name_lower.contains(query_lower) {
        score += 100;
    }

    // Query contains name
    if query_lower.contains(&name_lower) {
        score += 80;
    }

    // Keyword matching against name
    for kw in keywords {
        if name_lower.contains(kw) {
            score += 30;
        }
    }

    score
}

#[async_trait]
impl Tool for ToolSearchTool {
    fn name(&self) -> &str {
        "ToolSearch"
    }

    async fn description(&self, _input: &Value) -> String {
        "Fetches full schema definitions for deferred tools so they can be called.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Query to find tools. Use \"select:<tool_name>\" for direct selection, or keywords to search."
                },
                "max_results": {
                    "type": "number",
                    "default": 5,
                    "description": "Maximum number of results to return (default: 5)"
                }
            },
            "required": ["query"]
        })
    }

    fn is_concurrency_safe(&self, _input: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _input: &Value) -> bool {
        true
    }

    async fn call(
        &self,
        input: Value,
        ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let params: ToolSearchInput = serde_json::from_value(input)?;

        // Get available tools from the registry
        let tools = crate::tools::registry::get_all_tools();

        let matched_tools: Vec<Value>;

        if let Some(names_str) = params.query.strip_prefix("select:") {
            // Direct selection mode: "select:Read,Edit,Grep"
            let requested_names: Vec<&str> = names_str.split(',').map(|s| s.trim()).collect();

            matched_tools = tools
                .iter()
                .filter(|t| requested_names.iter().any(|name| t.name().eq_ignore_ascii_case(name)))
                .map(|t| tool_to_schema(t.as_ref()))
                .collect();

            debug!(
                query = %params.query,
                requested = requested_names.len(),
                found = matched_tools.len(),
                "ToolSearch: select mode"
            );
        } else {
            // Keyword search mode
            let query_lower = params.query.to_lowercase();
            let keywords: Vec<&str> = query_lower.split_whitespace().collect();

            let mut scored: Vec<(usize, &Arc<dyn Tool>)> = tools
                .iter()
                .map(|t| (score_tool(t.as_ref(), &query_lower, &keywords), t))
                .filter(|(score, _)| *score > 0)
                .collect();

            // Sort by score descending
            scored.sort_by(|a, b| b.0.cmp(&a.0));

            matched_tools = scored
                .into_iter()
                .take(params.max_results)
                .map(|(_, t)| tool_to_schema(t.as_ref()))
                .collect();

            debug!(
                query = %params.query,
                found = matched_tools.len(),
                max = params.max_results,
                "ToolSearch: keyword mode"
            );
        }

        if matched_tools.is_empty() {
            return Ok(ToolResult {
                data: json!({
                    "message": format!("No tools found matching '{}'", params.query),
                    "tools": []
                }),
                new_messages: vec![],
            });
        }

        Ok(ToolResult {
            data: json!({
                "message": format!("Found {} tool(s) matching '{}'", matched_tools.len(), params.query),
                "tools": matched_tools
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        "Use ToolSearch to find available tools by keyword or name.".to_string()
    }

    fn user_facing_name(&self, _input: Option<&Value>) -> String {
        "ToolSearch".to_string()
    }
}

/// Convert a Tool into a JSON schema representation for the search results.
fn tool_to_schema(tool: &dyn Tool) -> Value {
    json!({
        "name": tool.name(),
        "input_schema": tool.input_json_schema(),
        "read_only": tool.is_read_only(&json!({})),
        "concurrency_safe": tool.is_concurrency_safe(&json!({})),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_search_name() {
        let tool = ToolSearchTool;
        assert_eq!(tool.name(), "ToolSearch");
    }

    #[test]
    fn test_tool_search_schema() {
        let tool = ToolSearchTool;
        let schema = tool.input_json_schema();
        let props = schema["properties"].as_object().unwrap();
        assert!(props.contains_key("query"));
        assert!(props.contains_key("max_results"));
    }

    #[test]
    fn test_tool_search_concurrency_safe() {
        let tool = ToolSearchTool;
        assert!(tool.is_concurrency_safe(&json!({})));
        assert!(tool.is_read_only(&json!({})));
    }

    #[test]
    fn test_score_tool_exact_match() {
        // We can't easily create a tool instance here, but we can test
        // that the scoring function logic is correct by checking boundaries.
        // A tool with name "Bash" scored against "bash" should get 1000 (exact).
        struct MockTool;
        #[async_trait]
        impl Tool for MockTool {
            fn name(&self) -> &str { "Bash" }
            async fn description(&self, _: &Value) -> String { String::new() }
            fn input_json_schema(&self) -> Value { json!({}) }
            async fn call(&self, _: Value, _: &ToolUseContext, _: &AssistantMessage,
                _: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>) -> Result<ToolResult> {
                unreachable!()
            }
            async fn prompt(&self) -> String { String::new() }
        }
        let t = MockTool;
        assert_eq!(score_tool(&t, "bash", &["bash"]), 1000);
        assert!(score_tool(&t, "bas", &["bas"]) > 0); // partial match
        assert_eq!(score_tool(&t, "grep", &["grep"]), 0); // no match
    }
}

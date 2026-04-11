//! WebSearchTool implementation.

use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

use super::providers::{detect_provider, search_brave, search_tavily};
use super::{
    build_cache_key, cache_ttl_secs, filter_results_unified, format_results_text,
    SearchProvider, SearchResultEntry, BRAVE_API_KEY_ENV, DEFAULT_MAX_RESULTS,
    MAX_QUERY_LENGTH, MAX_RESULTS_CAP, SEARCH_CACHE, SEARCH_TIMEOUT, TAVILY_API_KEY_ENV,
};

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

pub struct WebSearchTool;

impl WebSearchTool {
    /// Fetch raw results from the search provider.
    async fn fetch_results(
        &self,
        provider: &SearchProvider,
        query: &str,
        max_results: u32,
    ) -> Result<Vec<SearchResultEntry>> {
        let client = reqwest::Client::builder()
            .timeout(SEARCH_TIMEOUT)
            .build()
            .context("Failed to build HTTP client")?;

        match provider {
            SearchProvider::Tavily(key) => search_tavily(&client, key, query, max_results).await,
            SearchProvider::Brave(key) => search_brave(&client, key, query, max_results).await,
        }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "WebSearch"
    }

    async fn description(&self, _: &Value) -> String {
        "Search the web for current information.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query (2-400 characters)."
                },
                "max_results": {
                    "type": "number",
                    "description": "Maximum number of results to return (default 5, max 20).",
                    "default": DEFAULT_MAX_RESULTS
                },
                "allowed_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Only return results from these domains."
                },
                "blocked_domains": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Exclude results from these domains."
                }
            },
            "required": ["query"]
        })
    }

    fn is_concurrency_safe(&self, _: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _: &Value) -> bool {
        true
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
        if query.len() < 2 {
            return ValidationResult::Error {
                message: "Search query must be at least 2 characters".to_string(),
                error_code: 400,
            };
        }
        if query.len() > MAX_QUERY_LENGTH {
            return ValidationResult::Error {
                message: format!("Search query exceeds {} character limit", MAX_QUERY_LENGTH),
                error_code: 400,
            };
        }
        ValidationResult::Ok
    }

    async fn call(
        &self,
        input: Value,
        _ctx: &ToolUseContext,
        _parent: &AssistantMessage,
        _on_progress: Option<Box<dyn Fn(ToolProgress) + Send + Sync>>,
    ) -> Result<ToolResult> {
        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");

        let max_results = input
            .get("max_results")
            .and_then(|v| v.as_u64())
            .map(|n| (n as u32).min(MAX_RESULTS_CAP))
            .unwrap_or(DEFAULT_MAX_RESULTS);

        let allowed_domains: Vec<String> = input
            .get("allowed_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let blocked_domains: Vec<String> = input
            .get("blocked_domains")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let provider = match detect_provider() {
            Some(p) => p,
            None => {
                return Ok(ToolResult {
                    data: json!({
                        "error": format!(
                            "WebSearch requires either {} or {} environment variable to be set.",
                            TAVILY_API_KEY_ENV, BRAVE_API_KEY_ENV
                        )
                    }),
                    new_messages: vec![],
                });
            }
        };

        let start = Instant::now();

        let provider_name = match &provider {
            SearchProvider::Tavily(_) => "tavily",
            SearchProvider::Brave(_) => "brave",
        };

        let ttl = cache_ttl_secs();
        let cache_key = build_cache_key(query, max_results, provider_name);

        // Check cache first, fetch on miss
        let raw_results = if ttl > 0 {
            if let Some(cached) = SEARCH_CACHE.get(&cache_key, ttl) {
                cached
            } else {
                match self.fetch_results(&provider, query, max_results).await {
                    Ok(results) => {
                        SEARCH_CACHE.put(&cache_key, results.clone());
                        results
                    }
                    Err(e) => {
                        return Ok(ToolResult {
                            data: json!({
                                "error": format!("{} search failed: {}", provider_name, e),
                                "query": query,
                            }),
                            new_messages: vec![],
                        });
                    }
                }
            }
        } else {
            // TTL=0 disables cache
            match self.fetch_results(&provider, query, max_results).await {
                Ok(results) => results,
                Err(e) => {
                    return Ok(ToolResult {
                        data: json!({
                            "error": format!("{} search failed: {}", provider_name, e),
                            "query": query,
                        }),
                        new_messages: vec![],
                    });
                }
            }
        };

        let results = filter_results_unified(raw_results, &allowed_domains, &blocked_domains);
        let duration_secs = start.elapsed().as_secs_f64();
        let results_text = format_results_text(&results);

        Ok(ToolResult {
            data: json!({
                "query": query,
                "provider": provider_name,
                "resultCount": results.len(),
                "results": results,
                "formattedResults": results_text,
                "durationSeconds": (duration_secs * 100.0).round() / 100.0,
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        let now = chrono::Utc::now();
        let month_year = now.format("%B %Y");
        let provider = match detect_provider() {
            Some(SearchProvider::Tavily(_)) => "Tavily",
            Some(SearchProvider::Brave(_)) => "Brave",
            None => "none (API key required)",
        };
        format!(
            r#"Search the web for current information. The current date is {month_year}.
Search provider: {provider}.

When presenting search results, you MUST include a "Sources:" section at the end
with markdown hyperlinks to the sources used.

Parameters:
- query: The search query (required, 2-400 chars)
- max_results: Number of results (default 5, max 20)
- allowed_domains: Only return results from these domains
- blocked_domains: Exclude results from these domains

Example sources section:
Sources:
- [Title](https://example.com/page)"#
        )
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(q) = input.and_then(|v| v.get("query")).and_then(|v| v.as_str()) {
            let short = if q.len() > 30 { &q[..30] } else { q };
            format!("WebSearch(\"{}\")", short)
        } else {
            "WebSearch".to_string()
        }
    }
}

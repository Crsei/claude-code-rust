//! WebSearch tool — search the web via Tavily or Brave Search API.
//!
//! Corresponds to TypeScript: tools/WebSearchTool/WebSearchTool.ts
//!
//! Supports two providers (checked in order):
//!   1. Tavily Search API  — `TAVILY_API_KEY` env var
//!   2. Brave Search API   — `BRAVE_SEARCH_API_KEY` env var
//!
//! At least one API key must be set.

use std::time::Instant;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Tavily Search API endpoint.
const TAVILY_API_URL: &str = "https://api.tavily.com/search";

/// Tavily API key environment variable.
const TAVILY_API_KEY_ENV: &str = "TAVILY_API_KEY";

/// Brave Search API endpoint.
const BRAVE_API_URL: &str = "https://api.search.brave.com/res/v1/web/search";

/// Brave API key environment variable.
const BRAVE_API_KEY_ENV: &str = "BRAVE_SEARCH_API_KEY";

/// Maximum number of results to request.
const DEFAULT_MAX_RESULTS: u32 = 5;

/// Absolute cap on results per query.
const MAX_RESULTS_CAP: u32 = 20;

/// Maximum search query length.
const MAX_QUERY_LENGTH: usize = 400;

/// Request timeout.
const SEARCH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

// ---------------------------------------------------------------------------
// Brave Search response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct BraveSearchResponse {
    #[serde(default)]
    web: Option<BraveWebResults>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResults {
    #[serde(default)]
    results: Vec<BraveWebResult>,
}

#[derive(Debug, Deserialize)]
struct BraveWebResult {
    title: String,
    url: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    age: Option<String>,
}

// ---------------------------------------------------------------------------
// Tavily Search response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TavilySearchResponse {
    #[serde(default)]
    results: Vec<TavilySearchResult>,
}

#[derive(Debug, Deserialize)]
struct TavilySearchResult {
    title: String,
    url: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    published_date: Option<String>,
}

// ---------------------------------------------------------------------------
// Search provider abstraction
// ---------------------------------------------------------------------------

/// Which search provider to use.
enum SearchProvider {
    Tavily(String),
    Brave(String),
}

fn detect_provider() -> Option<SearchProvider> {
    if let Ok(key) = std::env::var(TAVILY_API_KEY_ENV) {
        if !key.is_empty() {
            return Some(SearchProvider::Tavily(key));
        }
    }
    if let Ok(key) = std::env::var(BRAVE_API_KEY_ENV) {
        if !key.is_empty() {
            return Some(SearchProvider::Brave(key));
        }
    }
    None
}

/// Execute a Tavily search and return unified results.
async fn search_tavily(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    max_results: u32,
) -> Result<Vec<SearchResultEntry>> {
    let body = json!({
        "api_key": api_key,
        "query": query,
        "max_results": max_results,
        "search_depth": "basic",
    });

    let resp = client
        .post(TAVILY_API_URL)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("Tavily Search API request failed")?;

    let status = resp.status().as_u16();
    if status != 200 {
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Tavily Search API returned HTTP {}: {}", status, err_body);
    }

    let tavily_resp: TavilySearchResponse = resp
        .json()
        .await
        .context("Failed to parse Tavily Search response")?;

    Ok(tavily_resp
        .results
        .into_iter()
        .map(|r| SearchResultEntry {
            title: r.title,
            url: r.url,
            description: r.content,
            age: r.published_date,
        })
        .collect())
}

/// Execute a Brave search and return unified results.
async fn search_brave(
    client: &reqwest::Client,
    api_key: &str,
    query: &str,
    max_results: u32,
) -> Result<Vec<SearchResultEntry>> {
    let resp = client
        .get(BRAVE_API_URL)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .header("X-Subscription-Token", api_key)
        .query(&[("q", query), ("count", &max_results.to_string())])
        .send()
        .await
        .context("Brave Search API request failed")?;

    let status = resp.status().as_u16();
    if status != 200 {
        let err_body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Brave Search API returned HTTP {}: {}", status, err_body);
    }

    let search_resp: BraveSearchResponse = resp
        .json()
        .await
        .context("Failed to parse Brave Search response")?;

    let raw_results = search_resp.web.map(|w| w.results).unwrap_or_default();
    Ok(raw_results
        .into_iter()
        .map(|r| SearchResultEntry {
            title: r.title,
            url: r.url,
            description: r.description,
            age: r.age,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Result formatting
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct SearchResultEntry {
    title: String,
    url: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    age: Option<String>,
}

fn format_results_text(results: &[SearchResultEntry]) -> String {
    if results.is_empty() {
        return "No results found.".to_string();
    }
    let mut lines = Vec::new();
    for (i, r) in results.iter().enumerate() {
        lines.push(format!("{}. **{}**", i + 1, r.title));
        lines.push(format!("   {}", r.url));
        if !r.description.is_empty() {
            lines.push(format!("   {}", r.description));
        }
        if let Some(age) = &r.age {
            lines.push(format!("   ({})", age));
        }
        lines.push(String::new());
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Domain filtering
// ---------------------------------------------------------------------------

fn matches_domain(url: &str, domain: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        if let Some(host) = parsed.host_str() {
            return host == domain || host.ends_with(&format!(".{}", domain));
        }
    }
    false
}

fn filter_results(
    results: Vec<BraveWebResult>,
    allowed_domains: &[String],
    blocked_domains: &[String],
) -> Vec<SearchResultEntry> {
    results
        .into_iter()
        .filter(|r| {
            if !allowed_domains.is_empty() {
                return allowed_domains.iter().any(|d| matches_domain(&r.url, d));
            }
            if !blocked_domains.is_empty() {
                return !blocked_domains.iter().any(|d| matches_domain(&r.url, d));
            }
            true
        })
        .map(|r| SearchResultEntry {
            title: r.title,
            url: r.url,
            description: r.description,
            age: r.age,
        })
        .collect()
}

/// Filter unified SearchResultEntry results by domain rules.
fn filter_results_unified(
    results: Vec<SearchResultEntry>,
    allowed_domains: &[String],
    blocked_domains: &[String],
) -> Vec<SearchResultEntry> {
    results
        .into_iter()
        .filter(|r| {
            if !allowed_domains.is_empty() {
                return allowed_domains.iter().any(|d| matches_domain(&r.url, d));
            }
            if !blocked_domains.is_empty() {
                return !blocked_domains.iter().any(|d| matches_domain(&r.url, d));
            }
            true
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

pub struct WebSearchTool;

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

        let client = reqwest::Client::builder()
            .timeout(SEARCH_TIMEOUT)
            .build()
            .context("Failed to build HTTP client")?;

        let (provider_name, raw_results) = match &provider {
            SearchProvider::Tavily(key) => {
                match search_tavily(&client, key, query, max_results).await {
                    Ok(results) => ("tavily", results),
                    Err(e) => {
                        return Ok(ToolResult {
                            data: json!({
                                "error": format!("Tavily Search failed: {}", e),
                                "query": query,
                            }),
                            new_messages: vec![],
                        });
                    }
                }
            }
            SearchProvider::Brave(key) => {
                match search_brave(&client, key, query, max_results).await {
                    Ok(results) => ("brave", results),
                    Err(e) => {
                        return Ok(ToolResult {
                            data: json!({
                                "error": format!("Brave Search failed: {}", e),
                                "query": query,
                            }),
                            new_messages: vec![],
                        });
                    }
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_domain() {
        assert!(matches_domain(
            "https://docs.rust-lang.org/std",
            "rust-lang.org"
        ));
        assert!(matches_domain("https://rust-lang.org/", "rust-lang.org"));
        assert!(!matches_domain(
            "https://evil-rust-lang.org/",
            "rust-lang.org"
        ));
        assert!(!matches_domain("https://example.com/", "rust-lang.org"));
    }

    #[test]
    fn test_filter_results_allowed() {
        let results = vec![
            BraveWebResult {
                title: "Rust".into(),
                url: "https://rust-lang.org".into(),
                description: "".into(),
                age: None,
            },
            BraveWebResult {
                title: "Go".into(),
                url: "https://go.dev".into(),
                description: "".into(),
                age: None,
            },
        ];
        let filtered = filter_results(results, &["rust-lang.org".to_string()], &[]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Rust");
    }

    #[test]
    fn test_filter_results_blocked() {
        let results = vec![
            BraveWebResult {
                title: "A".into(),
                url: "https://spam.com/page".into(),
                description: "".into(),
                age: None,
            },
            BraveWebResult {
                title: "B".into(),
                url: "https://good.com/page".into(),
                description: "".into(),
                age: None,
            },
        ];
        let filtered = filter_results(results, &[], &["spam.com".to_string()]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "B");
    }

    #[test]
    fn test_format_results_empty() {
        let text = format_results_text(&[]);
        assert_eq!(text, "No results found.");
    }

    #[test]
    fn test_format_results_entries() {
        let results = vec![SearchResultEntry {
            title: "Example".into(),
            url: "https://example.com".into(),
            description: "An example site".into(),
            age: Some("2 days ago".into()),
        }];
        let text = format_results_text(&results);
        assert!(text.contains("**Example**"));
        assert!(text.contains("https://example.com"));
        assert!(text.contains("An example site"));
        assert!(text.contains("2 days ago"));
    }

    #[tokio::test]
    async fn test_web_search_tool_basics() {
        let tool = WebSearchTool;
        assert_eq!(tool.name(), "WebSearch");
        assert!(tool.is_read_only(&json!({})));
        assert!(tool.is_concurrency_safe(&json!({})));

        let schema = tool.input_json_schema();
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["max_results"].is_object());
        assert!(schema["properties"]["allowed_domains"].is_object());
    }

    #[tokio::test]
    async fn test_validate_input_short_query() {
        let tool = WebSearchTool;
        let ctx = make_test_ctx();
        let result = tool.validate_input(&json!({"query": "a"}), &ctx).await;
        match result {
            ValidationResult::Error { .. } => {}
            _ => panic!("expected error for short query"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_valid() {
        let tool = WebSearchTool;
        let ctx = make_test_ctx();
        let result = tool
            .validate_input(&json!({"query": "rust programming"}), &ctx)
            .await;
        match result {
            ValidationResult::Ok => {}
            _ => panic!("expected Ok for valid query"),
        }
    }

    #[test]
    fn test_user_facing_name() {
        let tool = WebSearchTool;
        let input = json!({"query": "how to write rust"});
        assert_eq!(
            tool.user_facing_name(Some(&input)),
            "WebSearch(\"how to write rust\")"
        );
    }

    #[test]
    fn test_detect_provider_tavily_first() {
        let orig_tavily = std::env::var(TAVILY_API_KEY_ENV).ok();
        let orig_brave = std::env::var(BRAVE_API_KEY_ENV).ok();

        std::env::set_var(TAVILY_API_KEY_ENV, "tvly-test");
        std::env::set_var(BRAVE_API_KEY_ENV, "brave-test");

        let provider = detect_provider();
        assert!(matches!(provider, Some(SearchProvider::Tavily(_))));

        // Restore
        match orig_tavily {
            Some(v) => std::env::set_var(TAVILY_API_KEY_ENV, v),
            None => std::env::remove_var(TAVILY_API_KEY_ENV),
        }
        match orig_brave {
            Some(v) => std::env::set_var(BRAVE_API_KEY_ENV, v),
            None => std::env::remove_var(BRAVE_API_KEY_ENV),
        }
    }

    #[test]
    fn test_detect_provider_brave_fallback() {
        let orig_tavily = std::env::var(TAVILY_API_KEY_ENV).ok();
        let orig_brave = std::env::var(BRAVE_API_KEY_ENV).ok();

        std::env::remove_var(TAVILY_API_KEY_ENV);
        std::env::set_var(BRAVE_API_KEY_ENV, "brave-test");

        let provider = detect_provider();
        assert!(matches!(provider, Some(SearchProvider::Brave(_))));

        match orig_tavily {
            Some(v) => std::env::set_var(TAVILY_API_KEY_ENV, v),
            None => std::env::remove_var(TAVILY_API_KEY_ENV),
        }
        match orig_brave {
            Some(v) => std::env::set_var(BRAVE_API_KEY_ENV, v),
            None => std::env::remove_var(BRAVE_API_KEY_ENV),
        }
    }

    #[test]
    fn test_detect_provider_none() {
        let orig_tavily = std::env::var(TAVILY_API_KEY_ENV).ok();
        let orig_brave = std::env::var(BRAVE_API_KEY_ENV).ok();

        std::env::remove_var(TAVILY_API_KEY_ENV);
        std::env::remove_var(BRAVE_API_KEY_ENV);

        let provider = detect_provider();
        assert!(provider.is_none());

        match orig_tavily {
            Some(v) => std::env::set_var(TAVILY_API_KEY_ENV, v),
            None => std::env::remove_var(TAVILY_API_KEY_ENV),
        }
        match orig_brave {
            Some(v) => std::env::set_var(BRAVE_API_KEY_ENV, v),
            None => std::env::remove_var(BRAVE_API_KEY_ENV),
        }
    }

    #[test]
    fn test_filter_results_unified_allowed() {
        let results = vec![
            SearchResultEntry {
                title: "Rust".into(),
                url: "https://rust-lang.org".into(),
                description: "".into(),
                age: None,
            },
            SearchResultEntry {
                title: "Go".into(),
                url: "https://go.dev".into(),
                description: "".into(),
                age: None,
            },
        ];
        let filtered =
            filter_results_unified(results, &["rust-lang.org".to_string()], &[]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "Rust");
    }

    #[test]
    fn test_tavily_response_deserialization() {
        let json_str = r#"{
            "results": [
                {
                    "title": "Rust Programming",
                    "url": "https://rust-lang.org",
                    "content": "A systems programming language",
                    "score": 0.95,
                    "published_date": "2024-01-15"
                }
            ]
        }"#;
        let resp: TavilySearchResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(resp.results.len(), 1);
        assert_eq!(resp.results[0].title, "Rust Programming");
        assert_eq!(resp.results[0].content, "A systems programming language");
        assert_eq!(
            resp.results[0].published_date.as_deref(),
            Some("2024-01-15")
        );
    }

    fn make_test_ctx() -> ToolUseContext {
        use crate::types::app_state::AppState;
        use std::sync::Arc;
        let state = AppState::default();
        ToolUseContext {
            options: ToolUseOptions {
                debug: false,
                main_loop_model: "test".into(),
                verbose: false,
                is_non_interactive_session: true,
                custom_system_prompt: None,
                append_system_prompt: None,
                max_budget_usd: None,
            },
            abort_signal: tokio::sync::watch::channel(false).1,
            read_file_state: FileStateCache::default(),
            get_app_state: Arc::new(move || state.clone()),
            set_app_state: Arc::new(|_| {}),
            messages: vec![],
            agent_id: None,
            agent_type: None,
            query_tracking: None,
            permission_callback: None,
            bg_agent_tx: None,
        }
    }
}

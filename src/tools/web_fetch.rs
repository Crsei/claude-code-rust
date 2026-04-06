//! WebFetch tool — fetch content from a URL and convert HTML to plain text.
//!
//! Corresponds to TypeScript: tools/WebFetchTool/WebFetchTool.ts
//!
//! Features:
//! - HTTP GET with configurable timeout
//! - HTML → plain-text extraction (strip tags)
//! - Content truncation (max 100 KB)
//! - LRU response cache (15 min TTL)
//! - URL validation and HTTPS upgrade

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::types::message::AssistantMessage;
use crate::types::tool::*;

// ---------------------------------------------------------------------------
// Constants (matching TypeScript source)
// ---------------------------------------------------------------------------

/// Maximum length of extracted text returned to the model.
const MAX_TEXT_LENGTH: usize = 100_000;

/// HTTP request timeout.
const FETCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Maximum response body size (10 MB).
const MAX_CONTENT_LENGTH: usize = 10 * 1024 * 1024;

/// Maximum URL length.
const MAX_URL_LENGTH: usize = 2_000;

/// Cache TTL (15 minutes).
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);

/// Maximum number of cached entries.
const MAX_CACHE_ENTRIES: usize = 64;

// ---------------------------------------------------------------------------
// Simple in-memory cache
// ---------------------------------------------------------------------------

struct CacheEntry {
    content: String,
    status: u16,
    fetched_at: Instant,
}

static CACHE: std::sync::LazyLock<Mutex<HashMap<String, CacheEntry>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

fn cache_get(url: &str) -> Option<(String, u16)> {
    let cache = CACHE.lock().ok()?;
    let entry = cache.get(url)?;
    if entry.fetched_at.elapsed() < CACHE_TTL {
        Some((entry.content.clone(), entry.status))
    } else {
        None
    }
}

fn cache_put(url: &str, content: &str, status: u16) {
    if let Ok(mut cache) = CACHE.lock() {
        // Evict oldest if over capacity
        if cache.len() >= MAX_CACHE_ENTRIES {
            let oldest_key = cache
                .iter()
                .min_by_key(|(_, v)| v.fetched_at)
                .map(|(k, _)| k.clone());
            if let Some(k) = oldest_key {
                cache.remove(&k);
            }
        }
        cache.insert(
            url.to_string(),
            CacheEntry {
                content: content.to_string(),
                status,
                fetched_at: Instant::now(),
            },
        );
    }
}

// ---------------------------------------------------------------------------
// HTML → text helpers
// ---------------------------------------------------------------------------

/// Naive HTML tag stripping — removes all `<…>` tags and decodes common
/// entities.  Good enough for extracting readable text from web pages without
/// pulling in a full HTML parser crate.
fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script = false;
    let mut in_style = false;
    let bytes = html.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        let ch = bytes[i];
        if ch == b'<' {
            // Detect <script and <style blocks
            let rest = &html[i..];
            if rest.len() > 7
                && rest[..7].eq_ignore_ascii_case("<script")
            {
                in_script = true;
            } else if rest.len() > 6
                && rest[..6].eq_ignore_ascii_case("<style")
            {
                in_style = true;
            } else if rest.len() > 8
                && rest[..9].eq_ignore_ascii_case("</script>")
            {
                in_script = false;
            } else if rest.len() > 7
                && rest[..8].eq_ignore_ascii_case("</style>")
            {
                in_style = false;
            }
            in_tag = true;
            i += 1;
            continue;
        }
        if ch == b'>' {
            in_tag = false;
            i += 1;
            continue;
        }
        if !in_tag && !in_script && !in_style {
            out.push(ch as char);
        }
        i += 1;
    }

    // Decode common HTML entities
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");

    // Collapse whitespace runs
    collapse_whitespace(&out)
}

/// Collapse runs of whitespace (spaces, tabs, newlines) into single spaces,
/// then collapse 3+ newlines into 2.
fn collapse_whitespace(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut prev_ws = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if ch == '\n' {
                if !prev_ws {
                    result.push('\n');
                }
                prev_ws = true;
            } else if !prev_ws {
                result.push(' ');
                prev_ws = true;
            }
        } else {
            prev_ws = false;
            result.push(ch);
        }
    }
    result.trim().to_string()
}

/// Truncate text to `max_len` characters with a notice if truncated.
fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }
    let half = max_len / 2;
    let head = &text[..half];
    let tail = &text[text.len() - half..];
    format!(
        "{}\n\n[... {} characters omitted ...]\n\n{}",
        head,
        text.len() - max_len,
        tail
    )
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Validate and normalise a URL string.
fn normalise_url(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        bail!("URL is empty");
    }
    if raw.len() > MAX_URL_LENGTH {
        bail!("URL exceeds {} character limit", MAX_URL_LENGTH);
    }

    // Upgrade http → https
    let url = if raw.starts_with("http://") {
        raw.replacen("http://", "https://", 1)
    } else if !raw.starts_with("https://") {
        format!("https://{}", raw)
    } else {
        raw.to_string()
    };

    // Basic parse check
    url::Url::parse(&url).context("Invalid URL")?;
    Ok(url)
}

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

pub struct WebFetchTool;

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "WebFetch"
    }

    async fn description(&self, _: &Value) -> String {
        "Fetch a web page and extract its text content.".to_string()
    }

    fn input_json_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch. Must be a valid HTTP/HTTPS URL."
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional instructions for content extraction (e.g. 'extract the main article text')."
                }
            },
            "required": ["url"]
        })
    }

    fn is_concurrency_safe(&self, _: &Value) -> bool {
        true
    }

    fn is_read_only(&self, _: &Value) -> bool {
        true
    }

    async fn validate_input(&self, input: &Value, _ctx: &ToolUseContext) -> ValidationResult {
        let url = input.get("url").and_then(|v| v.as_str()).unwrap_or("");
        if url.is_empty() {
            return ValidationResult::Error {
                message: "url is required".to_string(),
                error_code: 400,
            };
        }
        if url.len() > MAX_URL_LENGTH {
            return ValidationResult::Error {
                message: format!("URL exceeds {} character limit", MAX_URL_LENGTH),
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
        let raw_url = input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let _prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let url = normalise_url(raw_url)?;

        // Check cache first
        if let Some((cached, status)) = cache_get(&url) {
            return Ok(ToolResult {
                data: json!({
                    "url": url,
                    "status": status,
                    "content": cached,
                    "cached": true,
                }),
                new_messages: vec![],
            });
        }

        let start = Instant::now();

        let client = reqwest::Client::builder()
            .timeout(FETCH_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(10))
            .user_agent("ClaudeCode/0.1 (Rust)")
            .build()
            .context("Failed to build HTTP client")?;

        let resp = client
            .get(&url)
            .header("Accept", "text/html,application/xhtml+xml,text/plain,*/*")
            .send()
            .await
            .with_context(|| format!("Failed to fetch {}", url))?;

        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Read body with size limit
        let body_bytes = resp
            .bytes()
            .await
            .context("Failed to read response body")?;

        if body_bytes.len() > MAX_CONTENT_LENGTH {
            return Ok(ToolResult {
                data: json!({
                    "url": url,
                    "status": status,
                    "error": format!(
                        "Response body too large ({} bytes, limit {})",
                        body_bytes.len(),
                        MAX_CONTENT_LENGTH
                    ),
                }),
                new_messages: vec![],
            });
        }

        let body = String::from_utf8_lossy(&body_bytes).to_string();

        // Extract text from HTML or return raw
        let text = if content_type.contains("text/html")
            || content_type.contains("application/xhtml")
        {
            strip_html_tags(&body)
        } else {
            body
        };

        let text = truncate_text(&text, MAX_TEXT_LENGTH);
        let duration_ms = start.elapsed().as_millis() as u64;

        // Cache successful responses
        if (200..400).contains(&status) {
            cache_put(&url, &text, status);
        }

        Ok(ToolResult {
            data: json!({
                "url": url,
                "status": status,
                "content": text,
                "contentType": content_type,
                "bytes": body_bytes.len(),
                "durationMs": duration_ms,
            }),
            new_messages: vec![],
        })
    }

    async fn prompt(&self) -> String {
        r#"Fetch content from a URL and extract its text. The tool:
- Converts HTML to readable text (strips tags, scripts, styles)
- Truncates results to 100,000 characters
- Caches responses for 15 minutes
- Upgrades HTTP to HTTPS automatically
- Supports a `prompt` parameter for extraction instructions

Use this tool when you need to read web page content. For APIs returning JSON,
the raw response is returned as-is."#
            .to_string()
    }

    fn user_facing_name(&self, input: Option<&Value>) -> String {
        if let Some(url) = input.and_then(|v| v.get("url")).and_then(|v| v.as_str()) {
            // Show just the domain
            if let Ok(parsed) = url::Url::parse(url) {
                if let Some(host) = parsed.host_str() {
                    return format!("WebFetch({})", host);
                }
            }
            return format!("WebFetch({})", &url[..url.len().min(40)]);
        }
        "WebFetch".to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_html_basic() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let text = strip_html_tags(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
        assert!(!text.contains("<"));
    }

    #[test]
    fn test_strip_html_script_style() {
        let html = "<div>Before<script>var x = 1;</script>After<style>.a{}</style>End</div>";
        let text = strip_html_tags(html);
        assert!(text.contains("Before"));
        assert!(text.contains("After"));
        assert!(text.contains("End"));
        assert!(!text.contains("var x"));
        assert!(!text.contains(".a{"));
    }

    #[test]
    fn test_strip_html_entities() {
        let html = "A &amp; B &lt; C &gt; D &quot;E&quot; F&#39;s";
        let text = strip_html_tags(html);
        assert_eq!(text, "A & B < C > D \"E\" F's");
    }

    #[test]
    fn test_truncate_text_short() {
        let text = "short text";
        assert_eq!(truncate_text(text, 100), "short text");
    }

    #[test]
    fn test_truncate_text_long() {
        let text = "a".repeat(200);
        let result = truncate_text(&text, 100);
        assert!(result.contains("characters omitted"));
        assert!(result.len() < 200);
    }

    #[test]
    fn test_normalise_url_https() {
        let url = normalise_url("https://example.com").unwrap();
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn test_normalise_url_upgrade_http() {
        let url = normalise_url("http://example.com").unwrap();
        assert_eq!(url, "https://example.com");
    }

    #[test]
    fn test_normalise_url_add_scheme() {
        let url = normalise_url("example.com/path").unwrap();
        assert_eq!(url, "https://example.com/path");
    }

    #[test]
    fn test_normalise_url_empty() {
        assert!(normalise_url("").is_err());
    }

    #[test]
    fn test_normalise_url_too_long() {
        let long = "https://".to_string() + &"a".repeat(MAX_URL_LENGTH);
        assert!(normalise_url(&long).is_err());
    }

    #[test]
    fn test_cache_put_get() {
        cache_put("https://test-cache.example.com", "cached content", 200);
        let result = cache_get("https://test-cache.example.com");
        assert!(result.is_some());
        let (content, status) = result.unwrap();
        assert_eq!(content, "cached content");
        assert_eq!(status, 200);
    }

    #[test]
    fn test_cache_miss() {
        let result = cache_get("https://never-cached.example.com");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_web_fetch_tool_basics() {
        let tool = WebFetchTool;
        assert_eq!(tool.name(), "WebFetch");
        assert!(tool.is_read_only(&json!({})));
        assert!(tool.is_concurrency_safe(&json!({})));

        let schema = tool.input_json_schema();
        assert!(schema["properties"]["url"].is_object());
        assert!(schema["properties"]["prompt"].is_object());
    }

    #[test]
    fn test_user_facing_name() {
        let tool = WebFetchTool;
        let input = json!({"url": "https://docs.rs/reqwest/latest"});
        let name = tool.user_facing_name(Some(&input));
        assert_eq!(name, "WebFetch(docs.rs)");
    }
}

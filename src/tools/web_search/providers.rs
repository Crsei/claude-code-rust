//! Search provider implementations (Tavily, Brave).

use anyhow::{Context, Result};
use serde_json::json;

use super::{
    BraveSearchResponse, SearchProvider, SearchResultEntry, TavilySearchResponse,
    BRAVE_API_KEY_ENV, BRAVE_API_URL, TAVILY_API_KEY_ENV, TAVILY_API_URL,
};

pub(super) fn detect_provider() -> Option<SearchProvider> {
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
pub(super) async fn search_tavily(
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
pub(super) async fn search_brave(
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

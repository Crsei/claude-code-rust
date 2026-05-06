# Web Tools Provider Difference

## Scope

This note records the intentional implementation-path difference between Bun upstream web tools and cc-rust web tools.
It is a documentation closure for `architecture/tools-implementation-map.md`: WebSearch and WebFetch are implemented in cc-rust, but their provider/runtime path is not a byte-for-byte port of Bun upstream.

## Summary

| Tool | Bun upstream path | cc-rust path | Status |
| --- | --- | --- | --- |
| WebSearch | Anthropic beta server tool `web_search_20250305` passed through model streaming | Local Tavily or Brave Search API provider, then unified result formatting | Implemented, provider differs |
| WebFetch | Tool-managed URL fetch plus markdown extraction, with permission/domain checks | Local `reqwest` fetch plus sandbox network policy, URL normalization, same-origin redirect limit, response cache, binary rejection | Implemented, local policy path differs |

## WebSearch

Bun upstream builds an Anthropic `web_search_20250305` tool schema in `F:\AIclassmanager\cc\src\tools\WebSearchTool\WebSearchTool.ts` and sends the search request through `queryModelWithStreaming`.
The model/provider support gate lives in that same tool: first-party, supported Vertex Claude 4 models, and Foundry can enable the server tool.

cc-rust implements `WebSearch` in `crates/claude-code-rs/src/tools/web_search/`.
Provider resolution checks `TAVILY_API_KEY` first, then `BRAVE_SEARCH_API_KEY`.
The Rust tool sends the request directly to Tavily or Brave, normalizes the provider response into one result shape, applies `allowed_domains` / `blocked_domains`, and caches raw results before domain filtering.

This is an implementation-path difference, not a missing tool family.
The cc-rust behavior depends on local search provider credentials instead of Anthropic server-tool availability.

## WebFetch

Bun upstream `WebFetch` lives in `F:\AIclassmanager\cc\src\tools\WebFetchTool\`.
It accepts `url` and `prompt`, checks tool permissions by host/domain, fetches markdown content, handles cross-host redirects by returning a follow-up instruction, and applies the prompt to fetched content.

cc-rust implements `WebFetch` in `crates/claude-code-rs/src/tools/web_fetch.rs`.
The Rust path keeps the same user-facing tool name and input shape, then adds local execution boundaries:

- URL normalization upgrades bare/http URLs to HTTPS where applicable.
- Embedded credentials are rejected.
- Sandbox network policy is checked before fetching.
- Redirects are followed only when scheme, port, credentials, and normalized host remain compatible.
- Response content is cached in memory.
- Binary content is rejected instead of placing raw bytes into model context.

This is also an implementation-path difference, not a missing tool family.
If product requirements later demand Anthropic server-tool WebSearch parity specifically, that should be tracked as a separate provider decision, not as a generic tools implementation gap.

## Decision

Keep `architecture/tools-implementation-map.md` status for WebSearch / WebFetch as implemented.
Document the provider/runtime difference explicitly so future reviews distinguish capability parity from provider parity.

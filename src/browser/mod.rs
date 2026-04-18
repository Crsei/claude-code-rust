//! Browser MCP — identification and UX for self-hosted browser MCP servers.
//!
//! A "browser MCP server" is any MCP server (e.g. `mcp-chrome`, `mcp-playwright`,
//! or a user-rolled stdio server) that exposes browser-automation tools such as
//! `navigate`, `get_page_text`, `tabs_create`, `click`, `fill`, and so on.
//!
//! This module does not embed a browser. It only recognizes browser-shaped tool
//! capabilities coming in over MCP and adds four things on top:
//!
//! 1. **Detection** (`detection`) — mark a server as "browser" via explicit config
//!    (`"browserMcp": true` in settings.json) **or** by a tool-name heuristic.
//! 2. **Prompt guidance** (`prompt`) — inject a `# Browser Automation` section into
//!    the system prompt when at least one browser MCP server is active.
//! 3. **Permission categories** (`permissions`) — bucket browser actions into
//!    navigation / read / write / upload / js / observability with risk levels,
//!    so permission prompts read as "Allow navigating the browser to URL?" instead
//!    of "Allow tool 'mcp__chrome__navigate'?".
//! 4. **Tool result rendering** (`tool_rendering`) — classify common browser
//!    result shapes (screenshots, page dumps, console/network) and produce a
//!    compact one-line preview for the UI.

pub mod detection;
pub mod permissions;
pub mod prompt;
pub mod tool_rendering;

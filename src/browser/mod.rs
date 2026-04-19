//! Browser subsystem — two coexisting integrations with shared UX.
//!
//! The `browser/` module covers two related but independent ways cc-rust can
//! talk to a web browser:
//!
//! 1. **External browser MCP servers** (issues #2/#3). A user configures a
//!    third-party MCP server (e.g. `mcp-chrome`, `@playwright/mcp`) in
//!    `settings.json`; cc-rust identifies it and layers prompt guidance,
//!    permission categories, and structured result rendering on top.
//! 2. **First-party Chrome integration** (issues #4/#5). cc-rust ships its
//!    own native messaging host + MCP bridge and talks directly to the
//!    Anthropic Chrome extension — no third-party MCP server required.
//!
//! Both paths feed into the same downstream UX (prompt, permissions,
//! rendering), so they're grouped here rather than in separate crates.
//!
//! Module layout:
//!
//! - `detection` — heuristics for recognizing browser MCP tools (both paths).
//! - `permissions` — category / risk classification for permission prompts.
//! - `prompt` — `# Browser Automation` system-prompt section.
//! - `tool_rendering` — one-line previews for browser tool results.
//! - `common` — cross-platform Chromium browser paths + constants (#4+#5).
//! - `state` — runtime state for the first-party Chrome subsystem (#4+#5).
//! - `setup` — extension detection + native host manifest install (#4+#5).
//! - `session` — `ChromeSession` lifecycle (#4; transport lives in #5).

pub mod common;
pub mod detection;
pub mod permissions;
pub mod prompt;
pub mod session;
pub mod setup;
pub mod state;
pub mod tool_rendering;

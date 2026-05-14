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
//! Shared browser infrastructure lives in `cc-browser`; this root module keeps
//! only the live-tool detection adapter that still needs the root Tool list.

pub mod detection;

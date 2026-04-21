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
//! Phase 4 (issue #73) moved the parts of this module that did not touch
//! the `Tool` trait into the `cc-browser` workspace crate and re-exports
//! them here. `detection` and `prompt` still live locally because they
//! accept `Arc<dyn Tool>` — unblocked once the Tool trait leaves the root
//! crate (Phase 5 hub-cycle break).

pub use cc_browser::{
    common, mcp_bridge, native_host, permissions, session, state, tool_rendering,
};
// `setup` and `transport` are consumed by the CLI + integration tests via the
// full path `cc_browser::{setup,transport}::…`. Re-export them under the
// legacy `crate::browser::` names so any lingering call sites keep compiling.
#[allow(unused_imports)]
pub use cc_browser::{setup, transport};

pub mod detection;
pub mod prompt;

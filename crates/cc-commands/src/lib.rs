//! cc-commands — slash-command implementations (Phase 7 scaffold).
//!
//! Issue #76 (`[workspace-split] Phase 7`): target destination for
//! `crates/claude-code-rs/src/commands/` (53 commands, ~12.6k LOC — the
//! largest single-module extraction). This crate implements the
//! `CommandDispatcher` trait defined in cc-types::commands.
//!
//! Downstream deps (after full move): cc-engine, cc-plugins, cc-tools,
//! cc-teams, cc-browser, cc-compact, cc-session, cc-voice, cc-keybindings,
//! cc-mcp, cc-sandbox, cc-auth, cc-bootstrap, cc-skills, cc-utils.

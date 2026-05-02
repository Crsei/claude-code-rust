//! Placeholder: bottom pane container.
//!
//! Purpose:
//! - Own the prompt composer plus a stack of temporary views such as command
//!   popups, selection lists, approval overlays, MCP elicitation, and
//!   request-user-input prompts.
//! - Move focused input routing out of `App` while leaving process-level
//!   decisions such as quit and interrupt at the parent level.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/prompt_input.rs
//! - crates/claude-code-rs/src/ui/permissions.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/mod.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/bottom_pane_view.rs
//! - ui/src/components/PromptInput
//! - ui/src/components/permissions
//!
//! Implementation note:
//! - Start with a view-stack trait and one approval view before moving the
//!   composer into this module.


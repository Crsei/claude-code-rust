//! Placeholder: UI event router.
//!
//! Purpose:
//! - Split high-level terminal events, engine events, command results, and
//!   overlay responses out of the monolithic `App::handle_key_event` path.
//! - Provide the future boundary for busy/idle, queued submissions, abort,
//!   permission prompts, resize, and draw invalidation.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/app.rs
//! - crates/claude-code-rs/src/ui/tui.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/app.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/app_event.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/app_event_sender.rs
//!
//! Implementation note:
//! - Port the routing pattern, not Codex-specific protocol types.


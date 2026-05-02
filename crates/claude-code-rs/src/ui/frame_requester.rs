//! Placeholder: frame requester and redraw scheduling.
//!
//! Purpose:
//! - Introduce a narrow redraw scheduling primitive for time-based UI updates,
//!   streaming commits, transient footer hints, resize repaint, and overlay
//!   expiration.
//! - Reduce direct draw/tick coupling inside `tui.rs`.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/tui.rs
//! - crates/claude-code-rs/src/ui/terminal_env.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/tui/frame_requester.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/tui/frame_rate_limiter.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/tui.rs
//!
//! Implementation note:
//! - Keep the first implementation minimal: request redraw, coalesce redraw,
//!   and allow test-only inspection of pending requests.


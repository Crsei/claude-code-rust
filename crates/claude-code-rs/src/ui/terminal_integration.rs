//! Placeholder: terminal integration polish.
//!
//! Purpose:
//! - Centralize terminal title, notification backend selection, focus-aware
//!   notifications, external editor restore, mouse/copy behavior, no-flicker
//!   fallback, enhanced keys, zellij/tmux caveats, and alt-screen policy.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/tui.rs
//! - crates/claude-code-rs/src/ui/terminal_env.rs
//! - crates/claude-code-rs/src/ui/notifications
//! - ui/src/main.tsx
//! - ui/src/components/resize-sync.ts
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/tui.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/terminal_title.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/terminal_palette.rs
//!
//! Implementation note:
//! - Keep behavior opt-in where terminal support is uncertain.


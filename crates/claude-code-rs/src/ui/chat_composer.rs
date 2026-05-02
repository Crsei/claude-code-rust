//! Placeholder: chat composer.
//!
//! Purpose:
//! - Replace the small prompt-input-only surface with a richer composer that
//!   supports queued submissions, paste handling, history navigation, vim
//!   state, slash submodes, dynamic model choices, and status hints.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/prompt_input.rs
//! - crates/claude-code-rs/src/ui/vim.rs
//! - crates/claude-code-rs/src/ui/command_palette.rs
//! - ui/src/components/PromptInput
//! - ui/src/components/InputPrompt.tsx
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/chat_composer.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/chat_composer_history.rs
//!
//! Implementation note:
//! - Preserve current prompt behavior with regression tests before adding
//!   queue-while-busy or paste compaction.


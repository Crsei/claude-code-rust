//! Placeholder: tool activity and progress rendering.
//!
//! Purpose:
//! - Normalize tool use, tool result, shell progress, background task status,
//!   errors, cancellations, grouped read/search activity, and transcript
//!   detail rendering.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/messages.rs
//! - ui/src/adapters/tool-input.ts
//! - ui/src/adapters/tool-status.ts
//! - ui/src/store/message-model.ts
//! - ui/src/components/tasks
//! - ui/src/components/shell
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/exec_cell.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/exec_command.rs
//!
//! Implementation note:
//! - Keep prompt-view compact grouping and transcript-view full details as
//!   separate rendering modes.


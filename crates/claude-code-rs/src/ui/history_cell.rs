//! Placeholder: typed chat history cells.
//!
//! Purpose:
//! - Move from direct message-to-lines rendering toward typed cells for user,
//!   assistant, system, tool activity, progress, approval, diff, markdown,
//!   status, and collaboration events.
//! - Enable prompt-view grouping while keeping transcript-view detail.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/messages.rs
//! - crates/claude-code-rs/src/ui/transcript.rs
//! - ui/src/store/message-model.ts
//! - ui/src/components/messages
//! - ui/src/components/tasks
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/history_cell.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/exec_cell.rs
//!
//! Implementation note:
//! - Add cells behind the existing renderer first, then switch rendering one
//!   cell type at a time.


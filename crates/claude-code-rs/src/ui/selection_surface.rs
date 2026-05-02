//! Placeholder: selection, command, and picker surfaces.
//!
//! Purpose:
//! - Provide reusable UI for slash-command selection, file search, skill
//!   mentions, model/config choices, MCP server choices, multi-select lists,
//!   and narrow-terminal fallback rows.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/command_palette.rs
//! - crates/claude-code-rs/src/ui/browser.rs
//! - ui/src/components/design-system/FuzzyPicker.tsx
//! - ui/src/components/customselect
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/selection_list.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/list_selection_view.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/command_popup.rs
//!
//! Implementation note:
//! - This should become a generic primitive used by command palette and
//!   feature panels, not a command-specific widget.


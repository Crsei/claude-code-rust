//! Placeholder: typed approval and permission overlays.
//!
//! Purpose:
//! - Replace generic-only permission UI with typed approval surfaces for bash,
//!   file edit/write, web fetch, MCP approval/elicitation, request-user-input,
//!   and fallback permission requests.
//! - Preserve fail-closed behavior if a response path is missing.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - crates/claude-code-rs/src/ui/permissions.rs
//! - ui/src/components/permissions
//! - ui/src/components/permissions/PermissionRequestDialog.tsx
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/approval_overlay.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/mcp_server_elicitation.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/bottom_pane/request_user_input
//!
//! Implementation note:
//! - Start with a typed data model and snapshot/PTY coverage before changing
//!   the permission callback bridge.


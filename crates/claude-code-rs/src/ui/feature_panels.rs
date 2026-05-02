//! Placeholder: full-build feature panels.
//!
//! Purpose:
//! - Collect UI entry points for MCP, agents, teams, LSP diagnostics and
//!   recommendations, settings, sandbox, plugins, skills, background tasks,
//!   and subsystem status.
//! - Ensure every implemented backend feature has an accessible terminal UI
//!   path before experience-polish work begins.
//!
//! Reference paths:
//! - docs/ui-parity-update-plan.md
//! - ui/src/components/mcp
//! - ui/src/components/agents
//! - ui/src/components/agent-settings
//! - ui/src/components/TeamPanel.tsx
//! - ui/src/components/AgentTreePanel.tsx
//! - ui/src/components/LspRecommendationDialog.tsx
//! - ui/src/components/Settings
//! - ui/src/components/sandbox
//! - ui/src/components/panels
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/chatwidget/plugins.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/chatwidget/skills.rs
//! - F:/AIclassmanager/cc/codex/codex-rs/tui/src/multi_agents.rs
//!
//! Implementation note:
//! - Prefer thin panels backed by shared state snapshots over panels that call
//!   backend services directly.


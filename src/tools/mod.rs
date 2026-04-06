// Minimal tool system
//
// Core tools: Bash, FileRead, FileWrite, FileEdit, Glob, Grep, AskUser
// Skills: Skill tool for extensibility

pub mod orchestration;
pub mod hooks;
pub mod execution;
pub mod bash;
pub mod file_read;
pub mod file_write;
pub mod file_edit;
pub mod glob_tool;
pub mod grep;
pub mod ask_user;
pub mod registry;

// Skills tool
pub mod skill;

// Agent tool (Phase 2 migration)
pub mod agent;

// Web tools (Phase 3 migration)
pub mod web_fetch;
pub mod web_search;

// Plan mode + Task tools (Phase 4 migration)
pub mod plan_mode;
pub mod tasks;

// Worktree tools (Phase 5 migration)
pub mod worktree;

// LSP tool (Phase 7 migration)
pub mod lsp;

// SendMessage tool (Phase 8 — Teams inter-agent messaging)
pub mod send_message;

// Phase 14C: Additional tools
pub mod powershell;
pub mod config_tool;
pub mod repl;
pub mod structured_output;
pub mod send_user_message;

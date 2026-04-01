// Phase 2: Local tool system
//
// Tool registry + orchestration logic (concurrent/serial partitioning)
//
// Local tools:
//   BashTool, FileReadTool, FileWriteTool, FileEditTool, GlobTool, GrepTool,
//   NotebookEditTool, AskUserQuestionTool, ToolSearchTool

pub mod orchestration;
pub mod hooks;
pub mod execution;
pub mod bash;
pub mod file_read;
pub mod file_write;
pub mod file_edit;
pub mod glob_tool;
pub mod grep;
pub mod notebook_edit;
pub mod ask_user;
pub mod tool_search;
pub mod registry;

// Phase 8: Advanced local tools
pub mod agent;
pub mod tasks;
pub mod plan_mode;
pub mod worktree;
pub mod skill;

// Phase 12: Network tools (low priority)
pub mod web_fetch;
pub mod web_search;

// Phase 12: LSP tool
pub mod lsp;

// Agent Teams tools
pub mod team_create;
pub mod team_delete;
pub mod send_message;

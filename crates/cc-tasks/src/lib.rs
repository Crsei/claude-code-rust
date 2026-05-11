//! cc-tasks — task-domain boundary (Phase 6 workspace split scaffold).
//!
//! Target destination for the task store, task-output handling, task tools,
//! and task JSON/domain types currently living under
//! `crates/claude-code-rs/src/tools/tasks*`.
//!
//! This first boundary step owns only stable task-domain vocabulary so current
//! behavior stays in place while later moves can migrate implementation code
//! without changing schema strings.

pub mod domain;

pub use domain::{
    DEFAULT_TASK_LIST_ID, REMOTE_TASK_TYPE_AUTOFIX_PR, REMOTE_TASK_TYPE_BACKGROUND_PR,
    REMOTE_TASK_TYPE_ENUM, REMOTE_TASK_TYPE_REMOTE_AGENT, REMOTE_TASK_TYPE_ULTRAPLAN,
    REMOTE_TASK_TYPE_ULTRAREVIEW, TASK_CREATE_KIND_ENUM, TASK_KIND_DREAM,
    TASK_KIND_IN_PROCESS_TEAMMATE, TASK_KIND_LOCAL_AGENT, TASK_KIND_LOCAL_BASH,
    TASK_KIND_LOCAL_WORKFLOW, TASK_KIND_MONITOR_MCP, TASK_KIND_REMOTE_AGENT, TASK_KIND_TOOL,
};

//! cc-tasks — task-domain types, parsing, errors, and persistence shapes.
//!
//! Runtime store and tool adapters still live with the root runtime module
//! until their engine/UI/session dependencies can move cleanly.

pub mod commands;
pub mod domain;
pub mod errors;
pub mod tool_requests;
pub mod types;

pub use commands::{parse_tasks_command, TasksCommand};
pub use domain::{
    DEFAULT_TASK_LIST_ID, REMOTE_TASK_TYPE_AUTOFIX_PR, REMOTE_TASK_TYPE_BACKGROUND_PR,
    REMOTE_TASK_TYPE_ENUM, REMOTE_TASK_TYPE_REMOTE_AGENT, REMOTE_TASK_TYPE_ULTRAPLAN,
    REMOTE_TASK_TYPE_ULTRAREVIEW, TASK_CREATE_KIND_ENUM, TASK_KIND_DREAM,
    TASK_KIND_IN_PROCESS_TEAMMATE, TASK_KIND_LOCAL_AGENT, TASK_KIND_LOCAL_BASH,
    TASK_KIND_LOCAL_WORKFLOW, TASK_KIND_MONITOR_MCP, TASK_KIND_REMOTE_AGENT, TASK_KIND_TOOL,
};
pub use errors::{TaskError, TaskErrorCode};
pub use tool_requests::{
    dependency_ids_from_input, normalize_dependencies, normalize_optional_string,
    parse_task_create, parse_task_id, parse_task_update, string_array_field, task_id_from_input,
    task_update_fields_from_input, task_updated_fields_from_input, TaskCreateRequest,
    TaskUpdateAction, TaskUpdateRequest,
};
pub use types::{
    PersistedTaskFile, PersistedTaskRecord, TaskClaimFailure, TaskClaimFailureReason,
    TaskCreateOptions, TaskEntry, TaskOutputRetrievalStatus, TaskOutputWaitResult,
    TaskRuntimeHandle, TaskStatus, TaskUpdateFields, TeammateTaskExitReason, TodoItem,
    TodoWriteOutcome, UnassignTeammateTasksResult, UnassignedTaskSummary,
};

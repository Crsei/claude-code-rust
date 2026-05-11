//! Compatibility adapters for IPC client callback installation.

use std::sync::Arc;

use crate::engine::lifecycle::QueryEngine;

use super::protocol::BackendMessage;
use super::sink::FrontendSink;

/// Pending permission requests awaiting a response from the frontend.
pub type PendingPermissions = cc_ipc_client::PendingPermissions;
/// Pending AskUserQuestion requests awaiting the user's next submit_prompt.
pub type PendingQuestions = cc_ipc_client::PendingQuestions;

impl cc_ipc_client::callbacks::CallbackHost for QueryEngine {
    fn set_permission_callback(&self, cb: crate::types::tool::PermissionCallback) {
        QueryEngine::set_permission_callback(self, cb);
    }

    fn set_ask_user_callback(&self, cb: crate::types::tool::AskUserCallback) {
        QueryEngine::set_ask_user_callback(self, cb);
    }

    fn set_tool_progress_callback(
        &self,
        cb: Arc<dyn Fn(crate::types::tool::ToolProgress) + Send + Sync>,
    ) {
        QueryEngine::set_tool_progress_callback(self, cb);
    }
}

/// Install the permission callback on the engine.
pub fn install_permission_callback(
    engine: &Arc<QueryEngine>,
    pending: PendingPermissions,
    sink: FrontendSink,
) {
    let exit_plan_rejected = Arc::new(|engine: &QueryEngine, sink: &FrontendSink| {
        match crate::plan_workflow::reject_engine_plan(
            engine,
            "permission",
            Some("User rejected ExitPlanMode approval.".to_string()),
        ) {
            Ok(record) => {
                let _ = sink.send(&BackendMessage::PlanWorkflowEvent {
                    event: "approval_rejected".to_string(),
                    summary: crate::plan_workflow::summarize(&record),
                    record,
                });
            }
            Err(err) => {
                let _ = sink.send(&BackendMessage::SystemInfo {
                    text: format!("Plan approval was rejected, but workflow sync failed: {err}"),
                    level: "warning".to_string(),
                });
            }
        }
    });

    cc_ipc_client::callbacks::install_permission_callback(
        engine,
        pending,
        sink,
        Some(exit_plan_rejected),
    );
}

/// Install the `ToolProgress` callback on the engine.
pub fn install_tool_progress_callback(engine: &QueryEngine, sink: FrontendSink) {
    cc_ipc_client::callbacks::install_tool_progress_callback(engine, sink);
}

/// Install the AskUserQuestion callback on the engine.
pub fn install_ask_user_callback(
    engine: &QueryEngine,
    pending: PendingQuestions,
    sink: FrontendSink,
) {
    cc_ipc_client::callbacks::install_ask_user_callback(engine, pending, sink);
}

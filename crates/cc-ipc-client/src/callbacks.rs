//! Permission, question, and tool-progress callback builders.

use std::collections::HashMap;
use std::sync::Arc;

use cc_types::callbacks::{AskUserCallback, PermissionCallback, ToolProgress};
use cc_ipc_protocol::BackendMessage;
use parking_lot::Mutex;
use tokio::sync::oneshot;

use crate::sink::FrontendSink;

/// Pending permission requests awaiting a response from the frontend.
pub type PendingPermissions = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;
/// Pending AskUserQuestion requests awaiting a response from the frontend.
pub type PendingQuestions = Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>;

/// Host capability needed to install IPC callbacks.
pub trait CallbackHost {
    fn set_permission_callback(&self, cb: PermissionCallback);
    fn set_ask_user_callback(&self, cb: AskUserCallback);
    fn set_tool_progress_callback(&self, cb: Arc<dyn Fn(ToolProgress) + Send + Sync>);
}

/// Optional host hook invoked after the user rejects ExitPlanMode approval.
pub type ExitPlanRejectedHook<H> = Arc<dyn Fn(&H, &FrontendSink) + Send + Sync>;

pub fn install_permission_callback<H>(
    host: &Arc<H>,
    pending: PendingPermissions,
    sink: FrontendSink,
    exit_plan_rejected: Option<ExitPlanRejectedHook<H>>,
) where
    H: CallbackHost + Send + Sync + 'static,
{
    let host_handle = host.clone();
    let callback: PermissionCallback = Arc::new(
        move |tool_use_id: String, tool_name: String, description: String, options: Vec<String>| {
            let pending = pending.clone();
            let sink = sink.clone();
            let host = host_handle.clone();
            let exit_plan_rejected = exit_plan_rejected.clone();
            Box::pin(async move {
                let _ = sink.send(&BackendMessage::PermissionRequest {
                    tool_use_id: tool_use_id.clone(),
                    tool: tool_name.clone(),
                    command: description,
                    options,
                });

                let (tx, rx) = oneshot::channel();
                pending.lock().insert(tool_use_id, tx);

                match rx.await {
                    Ok(decision) => {
                        if tool_name == "ExitPlanMode"
                            && matches!(
                                decision.to_ascii_lowercase().as_str(),
                                "deny" | "reject" | "no"
                            )
                        {
                            if let Some(hook) = exit_plan_rejected.as_ref() {
                                hook(&host, &sink);
                            }
                        }
                        decision
                    }
                    Err(_) => "deny".to_string(),
                }
            })
        },
    );
    host.set_permission_callback(callback);
}

pub fn install_tool_progress_callback<H>(host: &H, sink: FrontendSink)
where
    H: CallbackHost + Send + Sync + 'static,
{
    let callback = Arc::new(move |progress: ToolProgress| {
        let data = &progress.data;
        let tool = data
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let output = data
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let elapsed_seconds = data
            .get("elapsed_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_lines = data.get("total_lines").and_then(|v| v.as_u64());
        let total_bytes = data.get("total_bytes").and_then(|v| v.as_u64());
        let timeout_ms = data.get("timeout_ms").and_then(|v| v.as_u64());

        let _ = sink.send(&BackendMessage::ToolProgress {
            tool_use_id: progress.tool_use_id,
            tool,
            output,
            elapsed_seconds,
            total_lines,
            total_bytes,
            timeout_ms,
        });
    });
    host.set_tool_progress_callback(callback);
}

pub fn install_ask_user_callback<H>(host: &H, pending: PendingQuestions, sink: FrontendSink)
where
    H: CallbackHost + Send + Sync + 'static,
{
    let callback: AskUserCallback = Arc::new(move |question: String| {
        let pending = pending.clone();
        let sink = sink.clone();
        Box::pin(async move {
            let question_id = uuid::Uuid::new_v4().to_string();
            let _ = sink.send(&BackendMessage::QuestionRequest {
                id: question_id.clone(),
                text: question,
            });

            let (tx, rx) = oneshot::channel();
            pending.lock().insert(question_id, tx);

            rx.await.unwrap_or_default()
        })
    });
    host.set_ask_user_callback(callback);
}

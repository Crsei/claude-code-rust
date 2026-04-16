//! Headless runtime — owns the `tokio::select!` main loop and all runtime state.
//!
//! [`HeadlessRuntime`] is the core object that ties together the engine,
//! permission/question bridges, the agent event bus, the subsystem event bus,
//! the prompt suggestion service, and the frontend sink.
//!
//! The previous monolithic `run_headless()` function is now a thin wrapper
//! that constructs a `HeadlessRuntime` and calls [`HeadlessRuntime::run()`].

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::io::AsyncBufReadExt;
use tracing::{debug, error, warn};

use crate::engine::lifecycle::QueryEngine;
use crate::services::prompt_suggestion::PromptSuggestionService;

use super::callbacks::{PendingPermissions, PendingQuestions};
use super::protocol::{BackendMessage, FrontendMessage};
use super::sink::FrontendSink;

/// The headless runtime.
///
/// All mutable runtime state lives here.  The `select!` loop in [`run()`]
/// multiplexes frontend messages, agent events, and subsystem events.
pub struct HeadlessRuntime {
    pub(crate) engine: Arc<QueryEngine>,
    pub(crate) pending_permissions: PendingPermissions,
    pub(crate) pending_questions: PendingQuestions,
    pub(crate) suggestion_svc: Arc<Mutex<PromptSuggestionService>>,
    pub(crate) sink: FrontendSink,
}

impl HeadlessRuntime {
    /// Create a new runtime.  Call [`run()`] to start the event loop.
    pub fn new(engine: Arc<QueryEngine>, sink: FrontendSink) -> Self {
        Self {
            engine,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            pending_questions: Arc::new(Mutex::new(HashMap::new())),
            suggestion_svc: Arc::new(Mutex::new(PromptSuggestionService::new(true))),
            sink,
        }
    }

    /// Run the headless event loop.
    ///
    /// Installs callbacks, sets up event channels, sends `Ready`, then enters
    /// the multiplexed `select!` loop.  Returns when the frontend sends `Quit`
    /// or stdin is closed.
    pub async fn run(&self, model: String) -> anyhow::Result<()> {
        // ── 1. Install callbacks ──────────────────────────────────────
        super::callbacks::install_permission_callback(
            &self.engine,
            self.pending_permissions.clone(),
            self.sink.clone(),
        );
        super::callbacks::install_ask_user_callback(
            &self.engine,
            self.pending_questions.clone(),
            self.sink.clone(),
        );

        // ── 1b. Background agent channel ─────────────────────────────
        let (agent_tx, mut agent_rx) = crate::ipc::agent_channel::agent_channel();
        self.engine.set_bg_agent_tx(agent_tx);
        let pending_bg = self.engine.pending_bg_results.clone();

        // ── 1c. Subsystem event bus ──────────────────────────────────
        let event_bus = super::subsystem_events::SubsystemEventBus::new();
        let mut event_rx = event_bus.subscribe();
        crate::lsp_service::set_event_sender(event_bus.sender());
        crate::mcp::set_event_sender(event_bus.sender());
        crate::plugins::set_event_sender(event_bus.sender());
        crate::skills::set_event_sender(event_bus.sender());

        // ── 2. Send Ready ────────────────────────────────────────────
        self.sink.send(&BackendMessage::Ready {
            session_id: self.engine.session_id.to_string(),
            model,
            cwd: self.engine.cwd().to_string(),
        })?;

        // ── 3. Main select loop ──────────────────────────────────────
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        let mut lines = stdin.lines();

        loop {
            tokio::select! {
                // ── Branch 1: Frontend message (stdin) ──────────────
                line = lines.next_line() => {
                    let line = match line {
                        Ok(Some(line)) => line,
                        Ok(None) => {
                            debug!("headless: stdin closed, exiting");
                            break;
                        }
                        Err(e) => {
                            error!("headless: error reading stdin: {}", e);
                            break;
                        }
                    };

                    let msg: FrontendMessage = match serde_json::from_str(&line) {
                        Ok(m) => m,
                        Err(e) => {
                            warn!(
                                "headless: failed to parse FrontendMessage: {} — line: {}",
                                e, line
                            );
                            let _ = self.sink.send(&BackendMessage::Error {
                                message: format!("invalid FrontendMessage: {}", e),
                                recoverable: true,
                            });
                            continue;
                        }
                    };

                    let keep_running = super::ingress::dispatch(
                        msg,
                        &self.engine,
                        &self.pending_permissions,
                        &self.pending_questions,
                        &self.suggestion_svc,
                        &self.sink,
                    ).await;

                    if !keep_running {
                        break;
                    }
                }

                // ── Branch 2: Agent/Team events ──────────────────────
                Some(event) = agent_rx.recv() => {
                    match event {
                        crate::ipc::agent_channel::AgentIpcEvent::Agent(ref agent_event) => {
                            if let crate::ipc::agent_events::AgentEvent::Completed {
                                ref agent_id, ref result_preview, had_error, duration_ms, ..
                            } = agent_event {
                                let tree = crate::ipc::agent_tree::AGENT_TREE.lock();
                                let (is_bg, desc) = tree.get(agent_id)
                                    .map(|n| (n.is_background, n.description.clone()))
                                    .unwrap_or((true, "unknown".to_string()));
                                drop(tree);

                                if is_bg {
                                    let _ = self.sink.send(&BackendMessage::BackgroundAgentComplete {
                                        agent_id: agent_id.clone(),
                                        description: desc.clone(),
                                        result_preview: result_preview.clone(),
                                        had_error: *had_error,
                                        duration_ms: *duration_ms,
                                    });
                                    pending_bg.push(crate::tools::background_agents::CompletedBackgroundAgent {
                                        agent_id: agent_id.clone(),
                                        description: desc,
                                        result_text: result_preview.clone(),
                                        had_error: *had_error,
                                        duration: std::time::Duration::from_millis(*duration_ms),
                                    });
                                }
                            }
                            let _ = self.sink.send(&BackendMessage::AgentEvent {
                                event: agent_event.clone(),
                            });
                        }
                        crate::ipc::agent_channel::AgentIpcEvent::Team(team_event) => {
                            let _ = self.sink.send(&BackendMessage::TeamEvent {
                                event: team_event,
                            });
                        }
                    }
                }

                // ── Branch 3: Subsystem events ───────────────────────
                Ok(event) = event_rx.recv() => {
                    let msg = match event {
                        super::subsystem_events::SubsystemEvent::Lsp(e) => {
                            BackendMessage::LspEvent { event: e }
                        }
                        super::subsystem_events::SubsystemEvent::Mcp(e) => {
                            BackendMessage::McpEvent { event: e }
                        }
                        super::subsystem_events::SubsystemEvent::Plugin(e) => {
                            BackendMessage::PluginEvent { event: e }
                        }
                        super::subsystem_events::SubsystemEvent::Skill(e) => {
                            BackendMessage::SkillEvent { event: e }
                        }
                    };
                    let _ = self.sink.send(&msg);
                }
            }
        }

        Ok(())
    }
}

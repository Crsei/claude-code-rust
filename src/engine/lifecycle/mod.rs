//! QueryEngine -- full session lifecycle implementation.
//!
//! Corresponds to TypeScript: QueryEngine.ts
//!
//! Owns a single conversation session. Implements the complete message dispatch
//! pipeline as described in QUERY_ENGINE_SESSION_LIFECYCLE.md:
//!
//!   Phase A: Input Processing
//!   Phase B: System Prompt Build
//!   Phase C: Pre-Query Setup (SystemInit, local-command fast path)
//!   Phase D: Query Loop -- full message dispatch (assistant, user, progress,
//!            system, attachment, stream, request_start, tombstone, tool_use_summary)
//!   Phase E: Result Generation (SdkResult)
//!
//! The stream returned by `submit_message` yields `SdkMessage` items.

mod deps;
mod helpers;
mod submit_message;
#[allow(clippy::module_inception)]
mod tests;
mod types;

// Re-export public types so external callers keep using
// `crate::engine::lifecycle::{QueryEngine, UsageTracking, ...}`
pub use types::{AbortReason, PermissionDenial, UsageTracking};

use parking_lot::RwLock;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tracing::info;

use crate::bootstrap::SessionId;
use crate::types::app_state::AppState;
use crate::types::config::QueryEngineConfig;
use crate::types::message::Message;
use crate::types::tool::Tools;

// ---------------------------------------------------------------------------
// QueryEngineState — consolidated mutable session state
// ---------------------------------------------------------------------------

/// All mutable session state behind a single `Arc<RwLock<_>>`.
///
/// Previously each field was an independent `Arc<Mutex<T>>` or `Arc<RwLock<T>>`,
/// requiring 10 individual clones in `submit_message`. Now there's one lock to
/// rule them all — simpler to reason about and fewer clones.
pub(crate) struct QueryEngineState {
    /// Conversation message history.
    pub(crate) messages: Vec<Message>,
    /// Abort reason (if aborted).
    pub(crate) abort_reason: Option<AbortReason>,
    /// Accumulated usage across all API calls.
    pub(crate) usage: UsageTracking,
    /// History of permission denials.
    pub(crate) permission_denials: Vec<PermissionDenial>,
    /// Total turn count across all `submit_message` invocations.
    pub(crate) total_turn_count: usize,
    /// Application-wide state (shared with deps).
    pub(crate) app_state: AppState,
    /// Current tool registry (shared with deps).
    pub(crate) tools: Tools,
    /// Skills discovered during this session (dedup).
    pub(crate) discovered_skill_names: HashSet<String>,
    /// Nested memory paths already loaded (dedup).
    pub(crate) loaded_nested_memory_paths: HashSet<String>,
    /// Async callback for interactive permission prompts (set by headless/TUI).
    pub(crate) permission_callback: Option<crate::types::tool::PermissionCallback>,
    /// Sender for background agent completion channel.
    /// Set by headless/TUI mode; cloned into ToolUseContext.
    pub(crate) bg_agent_tx: Option<crate::tools::background_agents::BgAgentSender>,
    /// If set, the engine is "sleeping" until this instant.
    /// The proactive tick loop skips ticks while `Instant::now() < sleep_until`.
    /// Cleared by `wake_up()` on user messages or external events.
    pub(crate) sleep_until: Option<std::time::Instant>,
}

// ---------------------------------------------------------------------------
// QueryEngine
// ---------------------------------------------------------------------------

/// QueryEngine -- owns the full lifecycle of a single conversation session.
///
/// Each session creates exactly one `QueryEngine`. It wraps the inner
/// `query::loop_impl::query()` generator, intercepting every yielded item to
/// maintain cross-turn state and produce `SdkMessage` items for the caller.
pub struct QueryEngine {
    /// Session identifier (UUID v4).
    pub session_id: SessionId,
    /// Immutable configuration snapshot.
    pub(crate) config: QueryEngineConfig,

    /// Consolidated mutable session state.
    pub(crate) state: Arc<RwLock<QueryEngineState>>,
    /// Atomic abort flag (fast path for the query loop — no lock needed).
    pub(crate) aborted: Arc<AtomicBool>,
    /// Whether we have handled the orphaned-permission edge case.
    pub(crate) has_handled_orphaned_permission: Arc<AtomicBool>,
    /// Shared buffer of completed background agents.
    /// Event loop pushes; query loop drains.
    pub(crate) pending_bg_results: crate::tools::background_agents::PendingBackgroundResults,
}

impl QueryEngine {
    // -- Construction --------------------------------------------------------

    /// Create a new QueryEngine with the given configuration.
    pub fn new(config: QueryEngineConfig) -> Self {
        let initial_messages = config.initial_messages.clone().unwrap_or_default();
        let tools = config.tools.clone();

        // Initialize AppState with resolved model from config
        let mut app_state = AppState::default();
        if let Some(ref model) = config.resolved_model {
            app_state.main_loop_model = model.clone();
            app_state.settings.model = Some(model.clone());
        }

        Self {
            session_id: SessionId::new(),
            config,
            state: Arc::new(RwLock::new(QueryEngineState {
                messages: initial_messages,
                abort_reason: None,
                usage: UsageTracking::default(),
                permission_denials: Vec::new(),
                total_turn_count: 0,
                app_state,
                tools,
                discovered_skill_names: HashSet::new(),
                loaded_nested_memory_paths: HashSet::new(),
                permission_callback: None,
                bg_agent_tx: None,
                sleep_until: None,
            })),
            aborted: Arc::new(AtomicBool::new(false)),
            has_handled_orphaned_permission: Arc::new(AtomicBool::new(false)),
            pending_bg_results: crate::tools::background_agents::PendingBackgroundResults::new(),
        }
    }

    // -- Permission callback --------------------------------------------------

    /// Set the async permission callback used by headless/TUI mode.
    /// When a tool requires `Ask` permission, this callback is invoked
    /// to prompt the user via IPC instead of immediately denying.
    pub fn set_permission_callback(&self, cb: crate::types::tool::PermissionCallback) {
        self.state.write().permission_callback = Some(cb);
    }

    /// Set the background agent sender (called by headless/TUI at startup).
    pub fn set_bg_agent_tx(&self, tx: crate::tools::background_agents::BgAgentSender) {
        self.state.write().bg_agent_tx = Some(tx);
    }

    // -- Sleep control -------------------------------------------------------

    /// Put the engine to sleep until the given instant.
    /// The proactive tick loop will skip ticks while `is_sleeping()` returns true.
    pub fn set_sleep_until(&self, until: std::time::Instant) {
        let mut state = self.state.write();
        state.sleep_until = Some(until);
    }

    /// Check whether the engine is currently sleeping.
    pub fn is_sleeping(&self) -> bool {
        let state = self.state.read();
        state.sleep_until.map_or(false, |t| std::time::Instant::now() < t)
    }

    /// Wake the engine up, clearing any pending sleep.
    /// Called on user messages, webhooks, or other external events.
    pub fn wake_up(&self) {
        let mut state = self.state.write();
        state.sleep_until = None;
    }

    // -- Abort control -------------------------------------------------------

    /// Abort the currently running query.
    pub fn abort(&self) {
        info!("aborting query engine");
        self.aborted.store(true, Ordering::SeqCst);
        self.state.write().abort_reason = Some(AbortReason::UserAbort);
    }

    /// Reset the abort flag before starting a new `submit_message` call.
    pub fn reset_abort(&self) {
        self.aborted.store(false, Ordering::SeqCst);
        self.state.write().abort_reason = None;
    }

    /// Check whether the engine has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    /// Get the abort reason (if any).
    pub fn abort_reason(&self) -> Option<AbortReason> {
        self.state.read().abort_reason.clone()
    }

    // -- Accessors -----------------------------------------------------------

    /// Get a snapshot of the current message history.
    pub fn messages(&self) -> Vec<Message> {
        self.state.read().messages.clone()
    }

    /// Get a snapshot of usage tracking.
    pub fn usage(&self) -> UsageTracking {
        self.state.read().usage.clone()
    }

    /// Get a snapshot of permission denials.
    pub fn permission_denials(&self) -> Vec<PermissionDenial> {
        self.state.read().permission_denials.clone()
    }

    /// Record a permission denial.
    pub fn record_permission_denial(&self, denial: PermissionDenial) {
        self.state.write().permission_denials.push(denial);
    }

    /// Get the total turn count (across all submit_message calls).
    pub fn total_turn_count(&self) -> usize {
        self.state.read().total_turn_count
    }

    /// Get a snapshot of the application state.
    pub fn app_state(&self) -> AppState {
        self.state.read().app_state.clone()
    }

    /// Update the application state with a closure.
    pub fn update_app_state<F>(&self, updater: F)
    where
        F: FnOnce(&mut AppState),
    {
        updater(&mut self.state.write().app_state);
    }

    /// Get the working directory.
    pub fn cwd(&self) -> &str {
        &self.config.cwd
    }

    /// Replace the tool registry.
    pub fn set_tools(&self, tools: Tools) {
        self.state.write().tools = tools;
    }

    /// Get discovered skill names from the current turn.
    pub fn discovered_skill_names(&self) -> HashSet<String> {
        self.state.read().discovered_skill_names.clone()
    }

    /// Get loaded nested memory paths.
    pub fn loaded_nested_memory_paths(&self) -> HashSet<String> {
        self.state.read().loaded_nested_memory_paths.clone()
    }
}

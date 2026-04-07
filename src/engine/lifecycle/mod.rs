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

mod types;
mod deps;
mod helpers;
mod submit_message;
#[allow(clippy::module_inception)]
mod tests;

// Re-export public types so external callers keep using
// `crate::engine::lifecycle::{QueryEngine, UsageTracking, ...}`
pub use types::{AbortReason, PermissionDenial, UsageTracking};

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use tracing::info;

use crate::bootstrap::SessionId;
use crate::types::app_state::AppState;
use crate::types::config::QueryEngineConfig;
use crate::types::message::Message;
use crate::types::tool::Tools;

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

    // -- Cross-turn persistent state (mutable via Arc wrappers) --------------

    /// Conversation message history.
    pub(crate) mutable_messages: Arc<RwLock<Vec<Message>>>,
    /// Abort reason (if aborted).
    pub(crate) abort_reason: Arc<Mutex<Option<AbortReason>>>,
    /// Atomic abort flag (fast path for the query loop).
    pub(crate) aborted: Arc<AtomicBool>,
    /// Accumulated usage across all API calls.
    pub(crate) usage: Arc<Mutex<UsageTracking>>,
    /// History of permission denials.
    pub(crate) permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    /// Total turn count across all `submit_message` invocations.
    pub(crate) total_turn_count: Arc<Mutex<usize>>,
    /// Application-wide state (shared with deps).
    pub(crate) app_state: Arc<RwLock<AppState>>,
    /// Current tool registry (shared with deps).
    pub(crate) tools: Arc<RwLock<Tools>>,

    // -- Session-level dedup / tracking --------------------------------------

    /// Skills discovered during this session (dedup).
    pub(crate) discovered_skill_names: Arc<Mutex<HashSet<String>>>,
    /// Nested memory paths already loaded (dedup).
    pub(crate) loaded_nested_memory_paths: Arc<Mutex<HashSet<String>>>,
    /// Whether we have handled the orphaned-permission edge case.
    pub(crate) has_handled_orphaned_permission: Arc<AtomicBool>,
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
            mutable_messages: Arc::new(RwLock::new(initial_messages)),
            abort_reason: Arc::new(Mutex::new(None)),
            aborted: Arc::new(AtomicBool::new(false)),
            usage: Arc::new(Mutex::new(UsageTracking::default())),
            permission_denials: Arc::new(Mutex::new(Vec::new())),
            total_turn_count: Arc::new(Mutex::new(0)),
            app_state: Arc::new(RwLock::new(app_state)),
            tools: Arc::new(RwLock::new(tools)),
            discovered_skill_names: Arc::new(Mutex::new(HashSet::new())),
            loaded_nested_memory_paths: Arc::new(Mutex::new(HashSet::new())),
            has_handled_orphaned_permission: Arc::new(AtomicBool::new(false)),
        }
    }

    // -- Abort control -------------------------------------------------------

    /// Abort the currently running query.
    pub fn abort(&self) {
        info!("aborting query engine");
        self.aborted.store(true, Ordering::SeqCst);
        *self.abort_reason.lock().expect("abort_reason lock poisoned") = Some(AbortReason::UserAbort);
    }

    /// Reset the abort flag before starting a new `submit_message` call.
    pub fn reset_abort(&self) {
        self.aborted.store(false, Ordering::SeqCst);
        *self.abort_reason.lock().expect("abort_reason lock poisoned") = None;
    }

    /// Check whether the engine has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.aborted.load(Ordering::Relaxed)
    }

    /// Get the abort reason (if any).
    pub fn abort_reason(&self) -> Option<AbortReason> {
        self.abort_reason.lock().expect("abort_reason lock poisoned").clone()
    }

    // -- Accessors -----------------------------------------------------------

    /// Get a snapshot of the current message history.
    pub fn messages(&self) -> Vec<Message> {
        self.mutable_messages.read().expect("messages lock poisoned").clone()
    }

    /// Get a snapshot of usage tracking.
    pub fn usage(&self) -> UsageTracking {
        self.usage.lock().expect("usage lock poisoned").clone()
    }

    /// Get a snapshot of permission denials.
    pub fn permission_denials(&self) -> Vec<PermissionDenial> {
        self.permission_denials.lock().expect("permission_denials lock poisoned").clone()
    }

    /// Record a permission denial.
    pub fn record_permission_denial(&self, denial: PermissionDenial) {
        self.permission_denials.lock().expect("permission_denials lock poisoned").push(denial);
    }

    /// Get the total turn count (across all submit_message calls).
    pub fn total_turn_count(&self) -> usize {
        *self.total_turn_count.lock().expect("turn_count lock poisoned")
    }

    /// Get a snapshot of the application state.
    pub fn app_state(&self) -> AppState {
        self.app_state.read().expect("app_state lock poisoned").clone()
    }

    /// Update the application state with a closure.
    pub fn update_app_state<F>(&self, updater: F)
    where
        F: FnOnce(&mut AppState),
    {
        let mut state = self.app_state.write().expect("app_state lock poisoned");
        updater(&mut state);
    }

    /// Get the working directory.
    pub fn cwd(&self) -> &str {
        &self.config.cwd
    }

    /// Replace the tool registry.
    pub fn set_tools(&self, tools: Tools) {
        *self.tools.write().expect("tools lock poisoned") = tools;
    }

    /// Get discovered skill names from the current turn.
    pub fn discovered_skill_names(&self) -> HashSet<String> {
        self.discovered_skill_names.lock().expect("discovered_skills lock poisoned").clone()
    }

    /// Get loaded nested memory paths.
    pub fn loaded_nested_memory_paths(&self) -> HashSet<String> {
        self.loaded_nested_memory_paths.lock().expect("loaded_memory lock poisoned").clone()
    }
}

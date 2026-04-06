//! In-process teammate execution backend.
//!
//! Corresponds to TypeScript: `utils/swarm/backends/InProcessBackend.ts`
//!
//! Teammates run as tokio tasks within the same process.
//! Uses `tokio::task_local!` for context isolation and
//! `CancellationToken` for lifecycle management.

#![allow(unused)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::backend::TeammateExecutor;
use super::constants::*;
use super::identity;
use super::mailbox;
use super::protocol;
use super::types::*;

use crate::types::tool::PermissionMode;

// ---------------------------------------------------------------------------
// In-process task registry
// ---------------------------------------------------------------------------

/// Global registry of in-process teammate tasks.
///
/// Shared across the application to track and manage spawned teammates.
static TASK_REGISTRY: std::sync::LazyLock<Mutex<HashMap<String, InProcessTeammateTaskState>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

// ---------------------------------------------------------------------------
// InProcessBackend
// ---------------------------------------------------------------------------

/// In-process execution backend — always available, no external dependencies.
pub struct InProcessBackend;

impl InProcessBackend {
    pub fn new() -> Self {
        Self
    }

    /// Look up a task by agent_id.
    fn find_task(agent_id: &str) -> Option<String> {
        let registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry
            .iter()
            .find(|(_, state)| state.identity.agent_id == agent_id)
            .map(|(id, _)| id.clone())
    }

    /// Get the number of running tasks.
    pub fn running_task_count() -> usize {
        let registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry
            .values()
            .filter(|s| s.status == TaskStatus::Running)
            .count()
    }

    /// Check if any in-process teammates are currently working (not idle).
    pub fn has_working_teammates() -> bool {
        let registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry
            .values()
            .any(|s| s.status == TaskStatus::Running && !s.is_idle)
    }

    /// Register a new task in the registry.
    pub fn register_task(state: InProcessTeammateTaskState) {
        let id = state.id.clone();
        let mut registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry.insert(id, state);
    }

    /// Update a task's status.
    pub fn update_task_status(task_id: &str, status: TaskStatus) {
        let mut registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = registry.get_mut(task_id) {
            state.status = status;
        }
    }

    /// Mark a task as idle or working.
    pub fn set_task_idle(task_id: &str, idle: bool) {
        let mut registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = registry.get_mut(task_id) {
            state.is_idle = idle;
        }
    }

    /// Remove a task from the registry.
    pub fn remove_task(task_id: &str) {
        let mut registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry.remove(task_id);
    }

    /// Get all task IDs.
    pub fn all_task_ids() -> Vec<String> {
        let registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry.keys().cloned().collect()
    }

    /// Clear all tasks (for testing).
    #[cfg(test)]
    pub fn clear_registry() {
        let mut registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry.clear();
    }
}

#[async_trait]
impl TeammateExecutor for InProcessBackend {
    fn backend_type(&self) -> BackendType {
        BackendType::InProcess
    }

    async fn is_available(&self) -> bool {
        true // Always available
    }

    async fn spawn(&self, config: TeammateSpawnConfig) -> Result<TeammateSpawnResult> {
        let agent_id = identity::format_agent_id(&config.name, &config.team_name);
        let task_id = format!(
            "in_process_teammate_{}",
            uuid::Uuid::new_v4().to_string()
        );

        let identity = TeammateIdentity {
            agent_id: agent_id.clone(),
            agent_name: config.name.clone(),
            team_name: config.team_name.clone(),
            color: config.color.clone(),
            plan_mode_required: config.plan_mode_required,
            parent_session_id: config.parent_session_id.clone(),
        };

        // Create task state
        let task_state = InProcessTeammateTaskState {
            id: task_id.clone(),
            status: TaskStatus::Running,
            identity: identity.clone(),
            prompt: config.prompt.clone(),
            model: config.model.clone(),
            abort_handle: None,
            awaiting_plan_approval: false,
            permission_mode: PermissionMode::Default,
            error: None,
            pending_user_messages: vec![],
            is_idle: false,
            shutdown_requested: false,
            last_reported_tool_count: 0,
            last_reported_token_count: 0,
        };

        // Register task
        Self::register_task(task_state);

        info!(
            agent_id = %agent_id,
            task_id = %task_id,
            "in-process teammate spawned"
        );

        Ok(TeammateSpawnResult {
            success: true,
            agent_id,
            error: None,
            abort_handle: None,
            task_id: Some(task_id),
            pane_id: None,
        })
    }

    async fn send_message(
        &self,
        agent_id: &str,
        team_name: &str,
        message: TeammateMessage,
    ) -> Result<()> {
        let (agent_name, _) = identity::parse_agent_id(agent_id)
            .ok_or_else(|| anyhow::anyhow!("invalid agent_id: {}", agent_id))?;
        mailbox::write_to_mailbox(&agent_name, message, team_name)
    }

    async fn terminate(&self, agent_id: &str, team_name: &str, reason: Option<&str>) -> bool {
        let task_id = match Self::find_task(agent_id) {
            Some(id) => id,
            None => {
                warn!(agent_id, "no task found for agent");
                return false;
            }
        };

        // Check if already requested
        {
            let registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = registry.get(&task_id) {
                if state.shutdown_requested {
                    debug!(agent_id, "shutdown already requested");
                    return true;
                }
            }
        }

        // Send shutdown request via mailbox
        let (agent_name, _) = match identity::parse_agent_id(agent_id) {
            Some(parts) => parts,
            None => return false,
        };

        let now = chrono::Utc::now();
        let request_id = protocol::shutdown_request_id(agent_id, now.timestamp());
        let shutdown_msg = serde_json::json!({
            "type": "shutdown_request",
            "requestId": request_id,
            "from": TEAM_LEAD_NAME,
            "reason": reason.unwrap_or("Team lead requested shutdown"),
            "timestamp": now.to_rfc3339(),
        });

        let message = TeammateMessage {
            from: TEAM_LEAD_NAME.into(),
            text: shutdown_msg.to_string(),
            timestamp: now.to_rfc3339(),
            read: false,
            color: None,
            summary: Some("Shutdown request".into()),
        };

        if let Err(e) = mailbox::write_to_mailbox(&agent_name, message, team_name) {
            warn!(error = %e, "failed to write shutdown request");
            return false;
        }

        // Mark as shutdown requested
        {
            let mut registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(state) = registry.get_mut(&task_id) {
                state.shutdown_requested = true;
            }
        }

        info!(agent_id, "shutdown request sent");
        true
    }

    async fn kill(&self, agent_id: &str) -> bool {
        let task_id = match Self::find_task(agent_id) {
            Some(id) => id,
            None => return false,
        };

        let mut registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = registry.get_mut(&task_id) {
            // Abort the task if we have a handle
            if let Some(ref handle) = state.abort_handle {
                handle.abort();
            }
            state.status = TaskStatus::Stopped;
            info!(agent_id, "teammate force-killed");
            true
        } else {
            false
        }
    }

    async fn is_active(&self, agent_id: &str) -> bool {
        let task_id = match Self::find_task(agent_id) {
            Some(id) => id,
            None => return false,
        };
        let registry = TASK_REGISTRY.lock().unwrap_or_else(|e| e.into_inner());
        registry
            .get(&task_id)
            .map(|s| s.status == TaskStatus::Running)
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() {
        InProcessBackend::clear_registry();
    }

    #[test]
    fn test_backend_type() {
        let backend = InProcessBackend::new();
        assert_eq!(backend.backend_type(), BackendType::InProcess);
    }

    #[tokio::test]
    async fn test_is_available() {
        let backend = InProcessBackend::new();
        assert!(backend.is_available().await);
    }

    #[tokio::test]
    async fn test_spawn_and_active() {
        setup();
        let backend = InProcessBackend::new();
        let config = TeammateSpawnConfig {
            name: "test-worker".into(),
            team_name: "test-team".into(),
            color: Some("blue".into()),
            plan_mode_required: false,
            prompt: "Do something".into(),
            cwd: ".".into(),
            model: None,
            system_prompt: None,
            system_prompt_mode: None,
            worktree_path: None,
            parent_session_id: "sess-1".into(),
            permissions: vec![],
            allow_permission_prompts: false,
        };

        let result = backend.spawn(config).await.unwrap();
        assert!(result.success);
        assert_eq!(result.agent_id, "test-worker@test-team");
        assert!(result.task_id.is_some());

        assert!(backend.is_active("test-worker@test-team").await);
        assert_eq!(InProcessBackend::running_task_count(), 1);

        setup(); // cleanup
    }

    #[tokio::test]
    async fn test_kill() {
        setup();
        let backend = InProcessBackend::new();
        let config = TeammateSpawnConfig {
            name: "kill-test".into(),
            team_name: "t".into(),
            color: None,
            plan_mode_required: false,
            prompt: "p".into(),
            cwd: ".".into(),
            model: None,
            system_prompt: None,
            system_prompt_mode: None,
            worktree_path: None,
            parent_session_id: "s".into(),
            permissions: vec![],
            allow_permission_prompts: false,
        };

        backend.spawn(config).await.unwrap();
        assert!(backend.is_active("kill-test@t").await);

        let killed = backend.kill("kill-test@t").await;
        assert!(killed);
        assert!(!backend.is_active("kill-test@t").await);

        setup();
    }

    #[test]
    fn test_has_working_teammates_empty() {
        setup();
        assert!(!InProcessBackend::has_working_teammates());
    }

    #[tokio::test]
    async fn test_kill_nonexistent() {
        setup();
        let backend = InProcessBackend::new();
        assert!(!backend.kill("nonexistent@team").await);
    }
}

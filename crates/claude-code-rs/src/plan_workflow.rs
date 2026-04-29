//! Plan-mode workflow service.
//!
//! The permission mode, markdown plan file, approval state, and trace record
//! all converge here so CLI, tools, headless IPC, and daemon routes do not
//! each invent their own plan-mode transition rules.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::paths as cfg_paths;
use crate::engine::lifecycle::QueryEngine;
use crate::types::app_state::AppState;
use crate::types::tool::PermissionMode;

pub use cc_types::plan_workflow::PlanWorkflowRecord;

#[cfg(test)]
use cc_types::plan_workflow::PlanWorkflowStatus;

const DEFAULT_OWNER: &str = "main";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntryClassifierDecision {
    pub should_enter: bool,
    pub reason: String,
    pub matched_rule: Option<String>,
}

/// Load the durable workflow record for `cwd`, if it exists.
pub fn load(cwd: &Path) -> Result<Option<PlanWorkflowRecord>> {
    let path = cfg_paths::current_plan_workflow_file_path(cwd);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read plan workflow: {}", path.display()))?;
    let record = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse plan workflow: {}", path.display()))?;
    Ok(Some(record))
}

/// Persist the durable workflow record for `cwd`.
pub fn persist(cwd: &Path, record: &PlanWorkflowRecord) -> Result<()> {
    let path = cfg_paths::current_plan_workflow_file_path(cwd);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create plan workflow dir: {}", parent.display()))?;
    }
    let raw = serde_json::to_string_pretty(record)?;
    fs::write(&path, raw)
        .with_context(|| format!("failed to write plan workflow: {}", path.display()))
}

/// Apply an enter-plan-mode transition to an [`AppState`] snapshot.
///
/// This function is intentionally side-effect-free except for mutating the
/// provided state; callers decide when to load and persist the record.
pub fn enter_plan_mode_state(
    app_state: &mut AppState,
    cwd: &Path,
    existing: Option<PlanWorkflowRecord>,
    owner: &str,
    source: &str,
    description: Option<&str>,
    classifier_reason: Option<&str>,
) -> PlanWorkflowRecord {
    if app_state.tool_permission_context.mode != PermissionMode::Plan {
        app_state.tool_permission_context.pre_plan_mode =
            Some(app_state.tool_permission_context.mode.clone());
        app_state.tool_permission_context.mode = PermissionMode::Plan;
    }

    let plan_file = cfg_paths::current_plan_file_path(cwd);
    let mut record = app_state
        .plan_workflow
        .clone()
        .or(existing)
        .unwrap_or_else(|| {
            PlanWorkflowRecord::new(display_path(&plan_file), Some(owner.to_string()), source)
        });

    record.file_path = display_path(&plan_file);
    if record.owner.is_none() {
        record.owner = Some(owner.to_string());
    }
    record.enter_plan_mode(source, description, classifier_reason);
    app_state.plan_workflow = Some(record.clone());
    record
}

pub fn request_approval_state(
    app_state: &mut AppState,
    cwd: &Path,
    existing: Option<PlanWorkflowRecord>,
    owner: &str,
    source: &str,
    plan_text: Option<String>,
) -> PlanWorkflowRecord {
    let mut record = ensure_record(app_state, cwd, existing, owner, source);
    record.request_approval(source, plan_text);
    app_state.plan_workflow = Some(record.clone());
    record
}

pub fn approve_and_exit_state(
    app_state: &mut AppState,
    cwd: &Path,
    existing: Option<PlanWorkflowRecord>,
    owner: &str,
    source: &str,
    plan_text: Option<String>,
) -> PlanWorkflowRecord {
    let mut record = ensure_record(app_state, cwd, existing, owner, source);
    if plan_text
        .as_deref()
        .is_some_and(|text| !text.trim().is_empty())
    {
        record.plan_text = plan_text;
    }
    record.approve(source);

    let restore_mode = app_state
        .tool_permission_context
        .pre_plan_mode
        .take()
        .unwrap_or(PermissionMode::Default);
    app_state.tool_permission_context.mode = restore_mode;
    app_state.plan_workflow = Some(record.clone());
    record
}

pub fn reject_approval_state(
    app_state: &mut AppState,
    cwd: &Path,
    existing: Option<PlanWorkflowRecord>,
    owner: &str,
    source: &str,
    feedback: Option<String>,
) -> PlanWorkflowRecord {
    let mut record = ensure_record(app_state, cwd, existing, owner, source);
    record.reject(source, feedback);
    app_state.plan_workflow = Some(record.clone());
    record
}

pub fn link_task_state(
    app_state: &mut AppState,
    cwd: &Path,
    existing: Option<PlanWorkflowRecord>,
    owner: &str,
    source: &str,
    task_id: String,
    summary: Option<String>,
) -> PlanWorkflowRecord {
    let mut record = ensure_record(app_state, cwd, existing, owner, source);
    record.link_task(source, task_id, summary);
    app_state.plan_workflow = Some(record.clone());
    record
}

pub fn enter_engine_plan_mode(
    engine: &QueryEngine,
    source: &str,
    description: Option<&str>,
    classifier_reason: Option<&str>,
) -> Result<PlanWorkflowRecord> {
    let cwd = PathBuf::from(engine.cwd());
    let existing = load(&cwd)?;
    let slot: Arc<Mutex<Option<PlanWorkflowRecord>>> = Arc::new(Mutex::new(None));
    let slot_for_update = Arc::clone(&slot);

    engine.update_app_state(|state| {
        let record = enter_plan_mode_state(
            state,
            &cwd,
            existing,
            DEFAULT_OWNER,
            source,
            description,
            classifier_reason,
        );
        *slot_for_update.lock().expect("plan workflow slot poisoned") = Some(record);
    });

    let record = slot
        .lock()
        .expect("plan workflow slot poisoned")
        .clone()
        .expect("plan workflow update should set record");
    persist(&cwd, &record)?;
    Ok(record)
}

pub fn reject_engine_plan(
    engine: &QueryEngine,
    source: &str,
    feedback: Option<String>,
) -> Result<PlanWorkflowRecord> {
    let cwd = PathBuf::from(engine.cwd());
    let existing = load(&cwd)?;
    let slot: Arc<Mutex<Option<PlanWorkflowRecord>>> = Arc::new(Mutex::new(None));
    let slot_for_update = Arc::clone(&slot);

    engine.update_app_state(|state| {
        let record = reject_approval_state(state, &cwd, existing, DEFAULT_OWNER, source, feedback);
        *slot_for_update.lock().expect("plan workflow slot poisoned") = Some(record);
    });

    let record = slot
        .lock()
        .expect("plan workflow slot poisoned")
        .clone()
        .expect("plan workflow update should set record");
    persist(&cwd, &record)?;
    Ok(record)
}

/// Sync state changed by a slash command back into the engine.
pub fn sync_command_app_state(engine: &QueryEngine, command_state: &AppState) {
    let permission_context = command_state.tool_permission_context.clone();
    let team_context = command_state.team_context.clone();
    let plan_workflow = command_state.plan_workflow.clone();
    engine.update_app_state(|state| {
        state.tool_permission_context = permission_context;
        state.team_context = team_context;
        state.plan_workflow = plan_workflow;
    });
}

/// Conservative classifier entry used before the full auto-mode LLM
/// classifier is ported. It only enters plan mode on explicit user wording.
pub fn classify_plan_entry(text: &str, app_state: &AppState) -> PlanEntryClassifierDecision {
    if app_state.tool_permission_context.mode == PermissionMode::Plan {
        return PlanEntryClassifierDecision {
            should_enter: false,
            reason: "plan mode is already active".to_string(),
            matched_rule: None,
        };
    }

    let trimmed = text.trim();
    if trimmed.starts_with('/') {
        return PlanEntryClassifierDecision {
            should_enter: false,
            reason: "slash commands own their own mode transitions".to_string(),
            matched_rule: None,
        };
    }

    let lower = trimmed.to_ascii_lowercase();
    for blocked in [
        "do not enter plan mode",
        "don't enter plan mode",
        "skip plan mode",
        "no plan mode",
    ] {
        if lower.contains(blocked) {
            return PlanEntryClassifierDecision {
                should_enter: false,
                reason: "explicit user override disables plan-mode classifier".to_string(),
                matched_rule: Some(blocked.to_string()),
            };
        }
    }

    for trigger in [
        "enter plan mode",
        "use plan mode",
        "plan mode first",
        "plan first",
        "make a plan first",
        "draft a plan first",
        "do not implement yet",
        "don't implement yet",
        "do not edit yet",
        "don't edit yet",
    ] {
        if lower.contains(trigger) {
            return PlanEntryClassifierDecision {
                should_enter: true,
                reason: "explicit user wording requested planning before edits".to_string(),
                matched_rule: Some(trigger.to_string()),
            };
        }
    }

    PlanEntryClassifierDecision {
        should_enter: false,
        reason: "no explicit plan-mode entry trigger matched".to_string(),
        matched_rule: None,
    }
}

pub fn summarize(record: &PlanWorkflowRecord) -> String {
    format!(
        "Plan workflow {}: status={:?}, approval={:?}, file={}, linked_tasks={}",
        record.id,
        record.status,
        record.approval_state,
        record.file_path,
        record.linked_task_ids.len()
    )
}

pub fn event_payload(record: &PlanWorkflowRecord, event: &str, summary: &str) -> serde_json::Value {
    json!({
        "event": event,
        "summary": summary,
        "record": record,
    })
}

fn ensure_record(
    app_state: &AppState,
    cwd: &Path,
    existing: Option<PlanWorkflowRecord>,
    owner: &str,
    source: &str,
) -> PlanWorkflowRecord {
    let plan_file = cfg_paths::current_plan_file_path(cwd);
    let mut record = app_state
        .plan_workflow
        .clone()
        .or(existing)
        .unwrap_or_else(|| {
            PlanWorkflowRecord::new(display_path(&plan_file), Some(owner.to_string()), source)
        });
    record.file_path = display_path(&plan_file);
    if record.owner.is_none() {
        record.owner = Some(owner.to_string());
    }
    record
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    #[serial]
    fn enter_plan_mode_sets_permission_and_persists_record_shape() {
        let tmp = tempdir().unwrap();
        let _g = EnvGuard::set("CC_RUST_HOME", tmp.path().to_str().unwrap());
        let mut state = AppState::default();

        let record = enter_plan_mode_state(
            &mut state,
            tmp.path(),
            None,
            "main",
            "test",
            Some("design"),
            None,
        );
        persist(tmp.path(), &record).unwrap();
        let loaded = load(tmp.path()).unwrap().unwrap();

        assert_eq!(state.tool_permission_context.mode, PermissionMode::Plan);
        assert_eq!(loaded.id, record.id);
        assert_eq!(loaded.status, PlanWorkflowStatus::Draft);
    }

    #[test]
    fn classifier_requires_explicit_plan_intent() {
        let state = AppState::default();
        let plain = classify_plan_entry("implement this feature", &state);
        assert!(!plain.should_enter);

        let explicit = classify_plan_entry("Plan first, then implement this feature", &state);
        assert!(explicit.should_enter);

        let blocked = classify_plan_entry("Do not enter plan mode; implement directly", &state);
        assert!(!blocked.should_enter);
    }
}

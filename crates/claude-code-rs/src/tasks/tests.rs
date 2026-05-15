use super::*;
use cc_tools::tool::ToolAppState as AppState;
use serde_json::json;
use std::ffi::OsString;
use std::sync::Arc;
use std::thread;

fn temp_store() -> (tempfile::TempDir, TaskStore) {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    (tmp, store)
}

fn temp_store_with_limit(limit: usize) -> (tempfile::TempDir, TaskStore) {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir_and_output_limit(tmp.path(), limit);
    (tmp, store)
}

fn test_context_with_app_state(app_state: AppState) -> ToolUseContext {
    let (_tx, rx) = tokio::sync::watch::channel(false);
    ToolUseContext {
        options: ToolUseOptions {
            debug: false,
            main_loop_model: "test".into(),
            verbose: false,
            is_non_interactive_session: false,
            custom_system_prompt: None,
            append_system_prompt: None,
            max_budget_usd: None,
        },
        abort_signal: rx,
        read_file_state: FileStateCache::default(),
        get_app_state: Arc::new(move || app_state.clone()),
        set_app_state: Arc::new(|_| {}),
        session_id: "test-session".to_string(),
        langfuse_session_id: "test-session".to_string(),
        messages: vec![],
        agent_id: None,
        agent_type: None,
        query_tracking: None,
        permission_callback: None,
        ask_user_callback: None,
        bg_agent_tx: None,
        hook_runner: Arc::new(cc_types::hooks::NoopHookRunner::new()),
        command_dispatcher: Arc::new(cc_types::commands::NoopCommandDispatcher::new()),
    }
}

fn test_context() -> ToolUseContext {
    test_context_with_app_state(AppState::default())
}

fn dummy_parent() -> AssistantMessage {
    AssistantMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: 0,
        role: "assistant".to_string(),
        content: vec![],
        usage: None,
        stop_reason: None,
        is_api_error_message: false,
        api_error: None,
        cost_usd: 0.0,
    }
}

struct EnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl AsRef<std::ffi::OsStr>) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, previous }
    }

    fn remove(key: &'static str) -> Self {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::remove_var(key);
        }
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

#[test]
#[serial_test::serial]
fn task_list_id_prefers_explicit_env_over_team_context() {
    let _cc_rust = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, " explicit/list ");
    let _claude = EnvGuard::remove(CLAUDE_CODE_TASK_LIST_ID_ENV);
    let _team_env = EnvGuard::set(CLAUDE_CODE_TEAM_NAME_ENV, "env-team");
    let mut app_state = AppState::default();
    app_state.team_context = Some(cc_types::teams::TeamContext {
        team_name: "state-team".to_string(),
        ..Default::default()
    });
    let ctx = test_context_with_app_state(app_state);

    assert_eq!(task_list_id_for_context(&ctx), "explicit/list");
    assert_eq!(sanitize_task_list_id("explicit/list"), "explicit-list");
}

#[test]
#[serial_test::serial]
fn task_list_id_uses_team_context_before_session_and_team_env() {
    let _cc_rust = EnvGuard::remove(CC_RUST_TASK_LIST_ID_ENV);
    let _claude = EnvGuard::remove(CLAUDE_CODE_TASK_LIST_ID_ENV);
    let _team_env = EnvGuard::set(CLAUDE_CODE_TEAM_NAME_ENV, "env-team");
    let mut app_state = AppState::default();
    app_state.team_context = Some(cc_types::teams::TeamContext {
        team_name: "state-team".to_string(),
        ..Default::default()
    });
    let ctx = test_context_with_app_state(app_state);

    assert_eq!(task_list_id_for_context(&ctx), "state-team");
}

#[tokio::test]
#[serial_test::serial]
async fn task_list_id_uses_in_process_teammate_team_name() {
    let _cc_rust = EnvGuard::remove(CC_RUST_TASK_LIST_ID_ENV);
    let _claude = EnvGuard::remove(CLAUDE_CODE_TASK_LIST_ID_ENV);
    let ctx = test_context();
    let identity = crate::teams::types::TeammateIdentity {
        agent_id: "worker@scope-team".to_string(),
        agent_name: "worker".to_string(),
        team_name: "scope-team".to_string(),
        color: None,
        plan_mode_required: false,
        parent_session_id: "leader-session".to_string(),
    };

    crate::teams::context::run_in_scope(identity, async {
        assert_eq!(task_list_id_for_context(&ctx), "scope-team");
    })
    .await;
}

#[tokio::test]
#[serial_test::serial]
async fn task_tools_use_task_list_scoped_store() {
    let home = tempfile::tempdir().unwrap();
    let _home = EnvGuard::set("CC_RUST_HOME", home.path());
    let list_a = format!("phase1-a-{}", uuid::Uuid::new_v4());
    let list_b = format!("phase1-b-{}", uuid::Uuid::new_v4());
    let parent = dummy_parent();

    {
        let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_a);
        let ctx_a = test_context();
        TaskCreateTool
            .call(
                json!({ "subject": "only list a", "description": "scoped" }),
                &ctx_a,
                &parent,
                None,
            )
            .await
            .expect("create task in list a");

        let listed = TaskListTool
            .call(json!({}), &ctx_a, &parent, None)
            .await
            .expect("list a");
        let tasks = listed.data["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["subject"], "only list a");
    }

    {
        let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_b);
        let ctx_b = test_context();
        let listed = TaskListTool
            .call(json!({}), &ctx_b, &parent, None)
            .await
            .expect("list b before create");
        assert_eq!(listed.data["count"], 0);

        TaskCreateTool
            .call(
                json!({ "subject": "only list b", "description": "scoped" }),
                &ctx_b,
                &parent,
                None,
            )
            .await
            .expect("create task in list b");
    }

    {
        let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_a);
        let ctx_a = test_context();
        let listed = TaskListTool
            .call(json!({}), &ctx_a, &parent, None)
            .await
            .expect("list a again");
        let tasks = listed.data["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["subject"], "only list a");
    }

    assert!(task_list_dir(&list_a).starts_with(home.path().join("tasks")));
    assert!(task_list_dir(&list_b).starts_with(home.path().join("tasks")));
}

#[test]
#[serial_test::serial]
fn default_task_list_store_copies_legacy_flat_tasks() {
    let home = tempfile::tempdir().unwrap();
    let _home = EnvGuard::set("CC_RUST_HOME", home.path());
    let legacy_root = home.path().join("tasks");
    fs::create_dir_all(&legacy_root).unwrap();
    fs::write(
        legacy_root.join("legacy-task.json"),
        serde_json::to_string_pretty(&json!({
            "id": "legacy-task",
            "subject": "legacy flat task",
            "description": "from pre-task-list storage",
            "status": "pending",
            "output": "legacy output",
            "created_at": 1,
            "updated_at": 2,
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(legacy_root.join(TASK_HIGHWATERMARK_FILE), "9\n").unwrap();

    let task_store = task_store_for_task_list_id(DEFAULT_TASK_LIST_ID);
    let migrated = task_store.get("legacy-task").unwrap();

    assert_eq!(migrated.subject, "legacy flat task");
    assert_eq!(migrated.output, "legacy output");
    assert!(legacy_root.join("legacy-task.json").exists());
    assert!(task_list_dir(DEFAULT_TASK_LIST_ID)
        .join("legacy-task.json")
        .exists());

    let next = task_store.create("next", "");
    assert_eq!(next.id, "10");
}

#[test]
fn test_task_store_create_and_get() {
    let (_tmp, store) = temp_store();
    let task = store.create("Test task", "Do the thing");
    assert_eq!(task.subject, "Test task");
    assert_eq!(task.description, "Do the thing");
    assert_eq!(task.status, TaskStatus::Pending);

    let fetched = store.get(&task.id).unwrap();
    assert_eq!(fetched.id, task.id);
}

#[test]
fn test_task_store_allocates_incrementing_ids_and_preserves_highwater() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let first = store.create("first", "");
    let second = store.create("second", "");

    assert_eq!(first.id, "1");
    assert_eq!(second.id, "2");

    store.delete(&second.id).unwrap();
    let third = store.create("third", "");
    assert_eq!(third.id, "3");

    let restarted = TaskStore::with_dir(tmp.path());
    let fourth = restarted.create("fourth", "");
    assert_eq!(fourth.id, "4");
}

#[test]
fn test_task_store_bootstraps_highwater_from_existing_numeric_tasks() {
    let tmp = tempfile::tempdir().unwrap();
    let legacy = TaskEntry {
        id: "7".to_string(),
        kind: TASK_KIND_TOOL.to_string(),
        subject: "legacy".to_string(),
        description: String::new(),
        status: TaskStatus::Completed,
        output: String::new(),
        output_summary: String::new(),
        output_bytes: 0,
        output_truncated: false,
        parent_id: None,
        depends_on: Vec::new(),
        owner: None,
        active_form: None,
        metadata: None,
        tool_use_id: None,
        agent_id: None,
        supervisor_id: None,
        isolation: None,
        worktree_path: None,
        worktree_branch: None,
        remote_task_type: None,
        remote_session_id: None,
        remote_task_metadata: None,
        poll_started_at: None,
        cancel_requested_at: None,
        recovered_at: None,
        previous_status: None,
        created_at: 1,
        updated_at: 1,
    };
    TaskRepository::new(tmp.path().to_path_buf(), DEFAULT_OUTPUT_LIMIT_BYTES)
        .persist_entry(&legacy)
        .unwrap();

    let store = TaskStore::with_dir(tmp.path());
    let next = store.create("next", "");
    assert_eq!(next.id, "8");
}

#[test]
fn test_concurrent_task_creates_use_unique_incrementing_ids() {
    let (_tmp, store) = temp_store();
    let store = Arc::new(store);
    let mut threads = Vec::new();

    for index in 0..8 {
        let store = Arc::clone(&store);
        threads.push(thread::spawn(move || {
            for _ in 0..20 {
                if let Ok(entry) = store.try_create(&format!("task {index}"), "") {
                    return entry.id;
                }
                thread::sleep(std::time::Duration::from_millis(10));
            }
            store
                .try_create(&format!("task {index}"), "")
                .expect("create after retries")
                .id
        }));
    }

    let mut ids: Vec<u64> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap().parse::<u64>().unwrap())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec![1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn test_task_store_persists_and_recovers_completed_task() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let task = store.create("Persist me", "survive restart");
    store.append_output(&task.id, "line 1");
    store.update_status(&task.id, TaskStatus::Completed);

    let restarted = TaskStore::with_dir(tmp.path());
    let fetched = restarted.get(&task.id).unwrap();
    assert_eq!(fetched.status, TaskStatus::Completed);
    assert_eq!(fetched.output, "line 1");
    assert_eq!(fetched.output_summary, "line 1");
}

#[test]
fn test_restart_marks_unfinished_tasks_interrupted() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let pending = store.create("pending", "restart");
    let running = store.create("running", "restart");
    store.update_status(&running.id, TaskStatus::InProgress);

    let restarted = TaskStore::with_dir(tmp.path());
    let pending = restarted.get(&pending.id).unwrap();
    let running = restarted.get(&running.id).unwrap();

    assert_eq!(pending.status, TaskStatus::Interrupted);
    assert_eq!(pending.previous_status, Some(TaskStatus::Pending));
    assert!(pending.recovered_at.is_some());
    assert_eq!(running.status, TaskStatus::Interrupted);
    assert_eq!(running.previous_status, Some(TaskStatus::InProgress));
}

#[test]
fn test_terminal_statuses_survive_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let completed = store.create("completed", "");
    let failed = store.create("failed", "");
    let cancelled = store.create("cancelled", "");
    store.update_status(&completed.id, TaskStatus::Completed);
    store.update_status(&failed.id, TaskStatus::Failed);
    store.stop(&cancelled.id);

    let restarted = TaskStore::with_dir(tmp.path());
    assert_eq!(
        restarted.get(&completed.id).unwrap().status,
        TaskStatus::Completed
    );
    assert_eq!(
        restarted.get(&failed.id).unwrap().status,
        TaskStatus::Failed
    );
    assert_eq!(
        restarted.get(&cancelled.id).unwrap().status,
        TaskStatus::Cancelled
    );
}

#[test]
fn test_task_store_update_status() {
    let (_tmp, store) = temp_store();
    let task = store.create("Update me", "...");

    let updated = store
        .update_status(&task.id, TaskStatus::InProgress)
        .unwrap();
    assert_eq!(updated.status, TaskStatus::InProgress);

    let completed = store
        .update_status(&task.id, TaskStatus::Completed)
        .unwrap();
    assert_eq!(completed.status, TaskStatus::Completed);
}

#[test]
fn test_task_store_list() {
    let (_tmp, store) = temp_store();
    store.create("Task A", "First");
    store.create("Task B", "Second");
    let list = store.list();
    assert_eq!(list.len(), 2);
}

#[test]
fn test_task_store_stop_cancels_runtime_handle() {
    let (_tmp, store) = temp_store();
    let task = store.create("Stop me", "...");
    let token = CancellationToken::new();
    assert!(store.register_runtime_handle(&task.id, token.clone()));

    let stopped = store.stop(&task.id).unwrap();
    assert_eq!(stopped.status, TaskStatus::Cancelled);
    assert!(stopped.cancel_requested_at.is_some());
    assert!(token.is_cancelled());
}

#[tokio::test]
#[serial_test::serial]
async fn task_list_and_task_stop_tools_cover_cancel_flow() {
    let subject = format!("phase0-stop-{}", uuid::Uuid::new_v4());
    let ctx = test_context();
    let task_store = store_for_context(&ctx);
    let task = task_store.create(&subject, "cancel through tool");
    let parent = dummy_parent();

    let listed = TaskListTool
        .call(json!({}), &ctx, &parent, None)
        .await
        .expect("list tasks");
    let tasks = listed.data["tasks"].as_array().expect("tasks array");
    assert!(tasks.iter().any(|entry| {
        entry["id"].as_str() == Some(task.id.as_str())
            && entry["subject"].as_str() == Some(subject.as_str())
            && entry["status"].as_str() == Some(TaskStatus::Pending.as_str())
    }));

    let stopped = TaskStopTool
        .call(json!({ "task_id": task.id.clone() }), &ctx, &parent, None)
        .await
        .expect("stop task");
    assert_eq!(stopped.data["task"]["id"].as_str(), Some(task.id.as_str()));
    assert_eq!(
        stopped.data["task"]["status"].as_str(),
        Some(TaskStatus::Cancelled.as_str())
    );
    assert!(stopped.data["message"].as_str().unwrap().contains(&subject));

    let missing = TaskStopTool
        .call(
            json!({ "task_id": format!("missing-{}", task.id) }),
            &ctx,
            &parent,
            None,
        )
        .await
        .expect("missing task response");
    assert!(missing.data["error"]
        .as_str()
        .unwrap()
        .contains("Task not found"));
}

#[test]
fn test_task_store_append_output() {
    let (_tmp, store) = temp_store();
    let task = store.create("Output task", "...");
    store.append_output(&task.id, "line 1");
    store.append_output(&task.id, "line 2");
    let entry = store.get(&task.id).unwrap();
    assert_eq!(entry.output, "line 1\nline 2");
    assert_eq!(entry.output_bytes, "line 1\nline 2".len());
}

#[test]
fn test_output_retention_is_bounded() {
    let (_tmp, store) = temp_store_with_limit(10);
    let task = store.create("Output task", "...");
    store.append_output(&task.id, "0123456789");
    store.append_output(&task.id, "abcdef");
    let entry = store.get(&task.id).unwrap();
    assert!(entry.output.len() <= 10);
    assert!(entry.output_truncated);
    assert!(entry.output.ends_with("abcdef"));
}

#[test]
fn test_dependencies_roundtrip_and_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let dep = store.create("dep", "");
    let child = store.create_with_options(
        "child",
        "",
        TaskCreateOptions {
            kind: Some("local_agent".to_string()),
            parent_id: Some(dep.id.clone()),
            depends_on: vec![dep.id.clone(), dep.id.clone()],
            ..TaskCreateOptions::default()
        },
    );

    assert_eq!(child.kind, "local_agent");
    assert_eq!(child.parent_id.as_deref(), Some(dep.id.as_str()));
    assert_eq!(child.depends_on, vec![dep.id.clone()]);
    assert_eq!(store.blocked_dependencies(&child), vec![dep.id.clone()]);
    assert_eq!(store.blocked_tasks(&dep), vec![child.id.clone()]);

    store.update_status(&dep.id, TaskStatus::Completed);
    let restarted = TaskStore::with_dir(tmp.path());
    let child = restarted.get(&child.id).unwrap();
    assert_eq!(child.depends_on, vec![dep.id.clone()]);
    assert!(restarted.blocked_dependencies(&child).is_empty());
}

#[test]
fn test_task_json_exposes_bun_dependency_aliases() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let dep = store.create("dep", "");
    let child = store.create_with_options(
        "child",
        "",
        TaskCreateOptions {
            depends_on: vec![dep.id.clone()],
            ..TaskCreateOptions::default()
        },
    );

    let dep_json = task_to_json_for_store(&store, &dep);
    assert_eq!(dep_json["blocks"], json!([child.id.clone()]));

    let child_json = task_to_json_for_store(&store, &child);
    assert_eq!(child_json["depends_on"], json!([dep.id.clone()]));
    assert_eq!(child_json["blocked_by"], json!([dep.id.clone()]));
    assert_eq!(child_json["blockedBy"], json!([dep.id.clone()]));
}

#[test]
fn test_task_claim_respects_dependencies_and_owner() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let dep = store.create("dep", "");
    let child = store.create_with_options(
        "child",
        "",
        TaskCreateOptions {
            depends_on: vec![dep.id.clone()],
            ..TaskCreateOptions::default()
        },
    );

    let blocked = store
        .claim_task(&child.id, "agent-a", false)
        .expect_err("unfinished dependency should block claim");
    assert_eq!(blocked.reason, TaskClaimFailureReason::Blocked);
    assert_eq!(blocked.blocked_by, vec![dep.id.clone()]);

    store.update_status(&dep.id, TaskStatus::Completed);
    let claimed = store.claim_task(&child.id, "agent-a", false).unwrap();
    assert_eq!(claimed.status, TaskStatus::InProgress);
    assert_eq!(claimed.owner.as_deref(), Some("agent-a"));

    let other_owner = store
        .claim_task(&child.id, "agent-b", false)
        .expect_err("different owner should not steal claim");
    assert_eq!(other_owner.reason, TaskClaimFailureReason::AlreadyClaimed);
    assert_eq!(other_owner.owner.as_deref(), Some("agent-a"));
}

#[test]
fn test_task_claim_agent_busy_mode_allows_one_unfinished_task_per_owner() {
    let (_tmp, store) = temp_store();
    let first = store.create("first", "");
    let second = store.create("second", "");

    store.claim_task(&first.id, "agent-a", true).unwrap();
    let busy = store
        .claim_task(&second.id, "agent-a", true)
        .expect_err("agent already owns an unfinished task");

    assert_eq!(busy.reason, TaskClaimFailureReason::AgentBusy);
    assert_eq!(busy.busy_task_id.as_deref(), Some(first.id.as_str()));

    store.update_status(&first.id, TaskStatus::Completed);
    let claimed = store.claim_task(&second.id, "agent-a", true).unwrap();
    assert_eq!(claimed.owner.as_deref(), Some("agent-a"));
}

#[test]
fn test_agent_metadata_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let task = store.create_with_options(
        "agent task",
        "metadata",
        TaskCreateOptions {
            kind: Some("local_agent".to_string()),
            owner: Some("agent-owner".to_string()),
            agent_id: Some("agent-1".to_string()),
            supervisor_id: Some("supervisor-1".to_string()),
            isolation: Some("worktree".to_string()),
            worktree_path: Some("/tmp/agent-worktree-abcd1234".to_string()),
            worktree_branch: Some("agent-worktree-abcd1234".to_string()),
            ..TaskCreateOptions::default()
        },
    );

    let by_agent = store.get_by_agent_id("agent-1").unwrap();
    assert_eq!(by_agent.id, task.id);
    assert_eq!(by_agent.owner.as_deref(), Some("agent-owner"));

    let restarted = TaskStore::with_dir(tmp.path());
    let restored = restarted.get_by_agent_id("agent-1").unwrap();
    assert_eq!(restored.owner.as_deref(), Some("agent-owner"));
    assert_eq!(restored.supervisor_id.as_deref(), Some("supervisor-1"));
    assert_eq!(restored.isolation.as_deref(), Some("worktree"));
    assert_eq!(
        restored.worktree_branch.as_deref(),
        Some("agent-worktree-abcd1234")
    );
}

#[test]
fn test_remote_task_metadata_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let task = store.create_with_options(
        "remote review",
        "poll remote session",
        TaskCreateOptions {
            kind: Some("remote-agent".to_string()),
            tool_use_id: Some("toolu_123".to_string()),
            remote_task_type: Some("remote_agent".to_string()),
            remote_session_id: Some("session-123".to_string()),
            remote_task_metadata: Some(json!({
                "owner": "acme",
                "repo": "widget",
                "prNumber": 42
            })),
            poll_started_at: Some(1_714_000_000_000),
            ..TaskCreateOptions::default()
        },
    );

    assert_eq!(task.kind, "remote_agent");
    assert_eq!(task.tool_use_id.as_deref(), Some("toolu_123"));
    assert_eq!(task.remote_task_type.as_deref(), Some("remote-agent"));
    assert_eq!(task.remote_session_id.as_deref(), Some("session-123"));
    assert_eq!(task.remote_task_metadata.as_ref().unwrap()["prNumber"], 42);
    assert_eq!(task.poll_started_at, Some(1_714_000_000_000));
    store.update_status(&task.id, TaskStatus::Completed);

    let restarted = TaskStore::with_dir(tmp.path());
    let restored = restarted.get(&task.id).unwrap();
    assert_eq!(restored.status, TaskStatus::Completed);
    assert_eq!(restored.kind, "remote_agent");
    assert_eq!(restored.tool_use_id.as_deref(), Some("toolu_123"));
    assert_eq!(restored.remote_task_type.as_deref(), Some("remote-agent"));
    assert_eq!(restored.remote_session_id.as_deref(), Some("session-123"));
    assert_eq!(
        restored.remote_task_metadata.as_ref().unwrap()["repo"],
        "widget"
    );
    assert_eq!(restored.poll_started_at, Some(1_714_000_000_000));
}

#[test]
fn test_restart_marks_remote_tasks_recoverable_and_resets_poll_timer() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let task = store.create_with_options(
        "remote agent",
        "resume remote session",
        TaskCreateOptions {
            kind: Some("remote_agent".to_string()),
            remote_task_type: Some("ultrareview".to_string()),
            remote_session_id: Some("session-restore".to_string()),
            remote_task_metadata: Some(json!({
                "owner": "acme",
                "repo": "widget",
                "prNumber": 7
            })),
            poll_started_at: Some(1_714_000_000_000),
            ..TaskCreateOptions::default()
        },
    );
    store.update_status(&task.id, TaskStatus::InProgress);

    let restarted = TaskStore::with_dir(tmp.path());
    let restored = restarted.get(&task.id).unwrap();

    assert_eq!(restored.status, TaskStatus::Recoverable);
    assert_eq!(restored.previous_status, Some(TaskStatus::InProgress));
    assert!(restored.recovered_at.is_some());
    assert_eq!(restored.remote_task_type.as_deref(), Some("ultrareview"));
    assert_eq!(
        restored.remote_session_id.as_deref(),
        Some("session-restore")
    );
    assert_eq!(
        restored.remote_task_metadata.as_ref().unwrap()["prNumber"],
        7
    );
    let poll_started_at = restored.poll_started_at.unwrap();
    assert!(poll_started_at > 1_714_000_000_000);

    let previous_recovered_at = restored.recovered_at;
    let restarted_again = TaskStore::with_dir(tmp.path());
    let restored_again = restarted_again.get(&task.id).unwrap();
    assert_eq!(restored_again.status, TaskStatus::Recoverable);
    assert_eq!(restored_again.previous_status, Some(TaskStatus::InProgress));
    assert_eq!(restored_again.recovered_at, previous_recovered_at);
    assert!(restored_again.poll_started_at.unwrap() >= poll_started_at);
}

#[test]
fn test_remote_review_timeout_marks_task_failed_and_persists() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let stale_poll_started_at =
        chrono::Utc::now().timestamp_millis() - REMOTE_REVIEW_TIMEOUT_MS - 1;
    let task = store.create_with_options(
        "remote review",
        "wait for remote review output",
        TaskCreateOptions {
            kind: Some("remote_agent".to_string()),
            remote_task_type: Some("ultrareview".to_string()),
            remote_session_id: Some("session-timeout".to_string()),
            poll_started_at: Some(stale_poll_started_at),
            ..TaskCreateOptions::default()
        },
    );
    store.update_status(&task.id, TaskStatus::InProgress);

    let timed_out = store.get(&task.id).unwrap();
    assert_eq!(timed_out.status, TaskStatus::Failed);
    assert_eq!(timed_out.previous_status, Some(TaskStatus::InProgress));
    assert!(timed_out
        .output
        .contains("remote session exceeded 30 minutes"));

    let restarted = TaskStore::with_dir(tmp.path());
    let restored = restarted.get(&task.id).unwrap();
    assert_eq!(restored.status, TaskStatus::Failed);
    assert!(restored
        .output
        .contains("remote session exceeded 30 minutes"));
}

#[test]
fn test_remote_review_timeout_does_not_apply_to_generic_remote_agents() {
    let (_tmp, store) = temp_store();
    let stale_poll_started_at =
        chrono::Utc::now().timestamp_millis() - REMOTE_REVIEW_TIMEOUT_MS - 1;
    let task = store.create_with_options(
        "remote agent",
        "wait for generic remote agent",
        TaskCreateOptions {
            kind: Some("remote_agent".to_string()),
            remote_task_type: Some("remote-agent".to_string()),
            remote_session_id: Some("session-generic".to_string()),
            poll_started_at: Some(stale_poll_started_at),
            ..TaskCreateOptions::default()
        },
    );
    store.update_status(&task.id, TaskStatus::InProgress);

    let still_running = store.get(&task.id).unwrap();
    assert_eq!(still_running.status, TaskStatus::InProgress);
}

#[test]
fn test_task_store_not_found() {
    let (_tmp, store) = temp_store();
    assert!(store.get("nonexistent").is_none());
    assert!(store
        .update_status("nonexistent", TaskStatus::Completed)
        .is_none());
    assert!(store.stop("nonexistent").is_none());
}

#[test]
fn test_todo_write_schema_matches_upstream_shape() {
    let schema = TodoWriteTool.input_json_schema();
    assert_eq!(schema["required"], json!(["todos"]));
    let item = &schema["properties"]["todos"]["items"];
    assert_eq!(item["required"], json!(["content", "status"]));
    assert_eq!(
        item["properties"]["status"]["enum"],
        json!(["pending", "in_progress", "completed"])
    );
    assert!(item["properties"].get("activeForm").is_some());
}

#[test]
fn test_todo_write_replaces_session_state_and_clears_completed_list() {
    let key = format!("todo-test-{}", uuid::Uuid::new_v4());
    let todos = parse_todo_items(&json!({
            "todos": [
                { "content": "Plan work", "status": "completed" },
                { "content": "Implement work", "status": "in_progress", "activeForm": "Implementing work" }
            ]
        }))
        .unwrap();

    let outcome = replace_todos_for_key(&key, todos);
    assert!(!outcome.cleared);
    assert_eq!(outcome.todos.len(), 2);
    assert_eq!(todo_snapshot_for_key(&key).len(), 2);

    let completed = parse_todo_items(&json!({
        "todos": [
            { "content": "Plan work", "status": "completed" },
            { "content": "Implement work", "status": "completed" },
            { "content": "Document work", "status": "completed" }
        ]
    }))
    .unwrap();

    let outcome = replace_todos_for_key(&key, completed);
    assert!(outcome.cleared);
    assert!(outcome.todos.is_empty());
    assert!(outcome.verification_nudge_needed);
    assert!(todo_snapshot_for_key(&key).is_empty());
}

#[test]
fn test_todo_write_keeps_completed_verification_list_quiet() {
    let key = format!("todo-test-{}", uuid::Uuid::new_v4());
    let todos = parse_todo_items(&json!({
        "todos": [
            { "content": "Implement work", "status": "completed" },
            { "content": "Verify behavior", "status": "completed" },
            { "content": "Update docs", "status": "completed" }
        ]
    }))
    .unwrap();

    let outcome = replace_todos_for_key(&key, todos);
    assert!(outcome.cleared);
    assert!(!outcome.verification_nudge_needed);
}

#[test]
fn test_todo_write_rejects_invalid_items() {
    assert!(parse_todo_items(&json!({})).is_err());
    assert!(parse_todo_items(&json!({
        "todos": [{ "content": "", "status": "pending" }]
    }))
    .is_err());
    assert!(parse_todo_items(&json!({
        "todos": [{ "content": "Run", "status": "running" }]
    }))
    .is_err());
}

#[test]
fn test_task_create_schema_uses_upstream_task_type_taxonomy() {
    let schema = TaskCreateTool.input_json_schema();
    let variants: Vec<&str> = schema["properties"]["kind"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        variants,
        vec![
            "tool",
            "local_bash",
            "local_agent",
            "remote_agent",
            "in_process_teammate",
            "local_workflow",
            "monitor_mcp",
            "dream",
        ]
    );
    assert!(!variants.contains(&"local_shell"));
    assert!(!variants.contains(&"workflow"));
    assert!(!variants.contains(&"team"));
}

#[test]
fn test_task_kind_aliases_canonicalize_to_upstream_types() {
    assert_eq!(sanitize_kind(" local_shell "), "local_bash");
    assert_eq!(sanitize_kind("local-bash"), "local_bash");
    assert_eq!(sanitize_kind("remote-agent"), "remote_agent");
    assert_eq!(sanitize_kind("team"), "in_process_teammate");
    assert_eq!(sanitize_kind("workflow"), "local_workflow");
    assert_eq!(sanitize_kind("monitor"), "monitor_mcp");
    assert_eq!(sanitize_kind("custom kind"), "custom_kind");
}

#[test]
fn test_remote_task_type_aliases_canonicalize_to_upstream_values() {
    assert_eq!(
        normalize_remote_task_type(Some("remote_agent".to_string())).as_deref(),
        Some("remote-agent")
    );
    assert_eq!(
        normalize_remote_task_type(Some("autofix_pr".to_string())).as_deref(),
        Some("autofix-pr")
    );
    assert_eq!(
        normalize_remote_task_type(Some("background-pr".to_string())).as_deref(),
        Some("background-pr")
    );
    assert_eq!(
        normalize_remote_task_type(Some("custom-remote".to_string())).as_deref(),
        Some("custom-remote")
    );
    assert_eq!(normalize_remote_task_type(Some(" ".to_string())), None);
}

#[test]
fn test_task_store_persists_canonicalized_task_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let shell = store.create_with_options(
        "shell",
        "",
        TaskCreateOptions {
            kind: Some("local_shell".to_string()),
            ..TaskCreateOptions::default()
        },
    );
    let workflow = store.create_with_options(
        "workflow",
        "",
        TaskCreateOptions {
            kind: Some("workflow".to_string()),
            ..TaskCreateOptions::default()
        },
    );

    assert_eq!(shell.kind, "local_bash");
    assert_eq!(workflow.kind, "local_workflow");

    let restarted = TaskStore::with_dir(tmp.path());
    assert_eq!(restarted.get(&shell.id).unwrap().kind, "local_bash");
    assert_eq!(restarted.get(&workflow.id).unwrap().kind, "local_workflow");
}

#[test]
fn test_task_create_schema_exposes_supervisor_metadata_fields() {
    let schema = TaskCreateTool.input_json_schema();
    let props = &schema["properties"];
    for field in [
        "depends_on",
        "blocked_by",
        "blockedBy",
        "owner",
        "tool_use_id",
        "agent_id",
        "supervisor_id",
        "remote_task_type",
        "remote_session_id",
        "remote_task_metadata",
        "poll_started_at",
    ] {
        assert!(props.get(field).is_some(), "schema should expose {field}");
    }
    let remote_types: Vec<&str> = props["remote_task_type"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        remote_types,
        vec![
            "remote-agent",
            "ultraplan",
            "ultrareview",
            "autofix-pr",
            "background-pr",
        ]
    );
}

#[test]
fn test_task_update_schema_exposes_claim_controls() {
    let schema = TaskUpdateTool.input_json_schema();
    let props = &schema["properties"];
    assert!(props.get("owner").is_some());
    assert!(props.get("check_agent_busy").is_some());
    assert!(props.get("checkAgentBusy").is_some());
}

#[test]
fn test_task_create_dependency_aliases_normalize() {
    let deps = dependency_ids_from_input(&json!({
        "depends_on": ["a", "b"],
        "blocked_by": ["b", "c"],
        "blockedBy": ["c", "d"]
    }));
    assert_eq!(deps, vec!["a", "b", "c", "d"]);
}

#[test]
fn test_task_output_schema_exposes_block_timeout_controls() {
    let schema = TaskOutputTool.input_json_schema();
    let props = &schema["properties"];
    assert_eq!(props["block"]["type"], "boolean");
    assert_eq!(props["block"]["default"], true);
    assert_eq!(props["timeout"]["type"], "integer");
    assert_eq!(props["timeout"]["minimum"], 0);
    assert_eq!(props["timeout"]["maximum"], MAX_TASK_OUTPUT_TIMEOUT_MS);
    assert_eq!(props["timeout"]["default"], DEFAULT_TASK_OUTPUT_TIMEOUT_MS);
}

#[test]
fn test_task_output_timeout_validation_matches_upstream_bounds() {
    assert_eq!(parse_task_output_timeout_ms(&json!({})).unwrap(), 30_000);
    assert_eq!(
        parse_task_output_timeout_ms(&json!({ "timeout": 600_000 })).unwrap(),
        600_000
    );
    assert!(parse_task_output_timeout_ms(&json!({ "timeout": 600_001 })).is_err());
    assert!(parse_task_output_timeout_ms(&json!({ "timeout": -1 })).is_err());
    assert!(parse_task_output_timeout_ms(&json!({ "timeout": 1.5 })).is_err());
}

#[test]
fn test_task_output_payload_preserves_legacy_fields_and_status() {
    let (_tmp, store) = temp_store();
    let task = store.create_with_options(
        "agent task",
        "collect output",
        TaskCreateOptions {
            kind: Some("local_agent".to_string()),
            agent_id: Some("agent-1".to_string()),
            supervisor_id: Some("supervisor-1".to_string()),
            ..TaskCreateOptions::default()
        },
    );
    store.update_status(&task.id, TaskStatus::InProgress);
    store.append_output(&task.id, "partial output");
    let entry = store.get(&task.id).unwrap();

    let payload = task_output_payload(&entry, TaskOutputRetrievalStatus::NotReady);
    assert_eq!(payload["retrieval_status"], "not_ready");
    assert_eq!(payload["task"]["task_id"], task.id);
    assert_eq!(payload["task"]["task_type"], "local_agent");
    assert_eq!(payload["task"]["status"], "in_progress");
    assert_eq!(payload["task"]["output"], "partial output");
    assert_eq!(payload["task_id"], task.id);
    assert_eq!(payload["output"], "partial output");
    assert_eq!(payload["agent_id"], "agent-1");
}

#[test]
fn phase0_gap_cross_store_claim_requires_task_list_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let store_a = TaskStore::with_dir(tmp.path());
    let store_b = TaskStore::with_dir(tmp.path());
    let task = store_a.create("claim race", "");

    let claimed_a = store_a
        .claim_task(&task.id, "agent-a", true)
        .expect("first claimant should win");
    assert_eq!(claimed_a.owner.as_deref(), Some("agent-a"));

    let claimed_b = store_b.claim_task(&task.id, "agent-b", true);
    assert!(
        claimed_b.is_err(),
        "second store must observe the persisted owner and fail the claim"
    );
}

#[test]
fn task_list_lock_makes_agent_busy_check_cross_store_atomic() {
    let tmp = tempfile::tempdir().unwrap();
    let store_a = TaskStore::with_dir(tmp.path());
    let store_b = TaskStore::with_dir(tmp.path());
    let first = store_a.create("first", "");
    let second = store_a.create("second", "");

    store_a
        .claim_task(&first.id, "agent-a", true)
        .expect("first claim should win");
    let busy = store_b.claim_task(&second.id, "agent-a", true).unwrap_err();

    assert_eq!(busy.reason, TaskClaimFailureReason::AgentBusy);
    assert_eq!(busy.busy_task_id.as_deref(), Some(first.id.as_str()));
}

#[test]
fn task_claim_reports_lock_unavailable_when_list_lock_is_held() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let task = store.create("locked", "");
    fs::write(tmp.path().join(TASK_LIST_LOCK_FILE), "").unwrap();

    let failure = store.claim_task(&task.id, "agent-a", true).unwrap_err();

    assert_eq!(failure.reason, TaskClaimFailureReason::LockUnavailable);
}

#[test]
fn task_write_paths_return_error_when_list_lock_is_held() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let task = store.create("locked", "");
    fs::write(tmp.path().join(TASK_LIST_LOCK_FILE), "").unwrap();

    assert!(store.try_create("new task", "").is_err());
    assert!(store
        .try_update_status(&task.id, TaskStatus::Completed)
        .is_err());
    assert!(store.try_delete(&task.id).is_err());
    assert!(store.try_stop(&task.id).is_err());

    let unchanged = store.get(&task.id).expect("task remains");
    assert_eq!(unchanged.status, TaskStatus::Pending);
}

#[test]
fn task_delete_removes_dependency_references_under_list_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TaskStore::with_dir(tmp.path());
    let blocker = store.create("blocker", "");
    let blocked = store.create_with_options(
        "blocked",
        "",
        TaskCreateOptions {
            depends_on: vec![blocker.id.clone()],
            ..TaskCreateOptions::default()
        },
    );

    store.delete(&blocker.id).expect("delete blocker");
    let reloaded = TaskStore::with_dir(tmp.path());
    let cleaned = reloaded.get(&blocked.id).unwrap();

    assert!(cleaned.depends_on.is_empty());
}

#[test]
fn task_list_refreshes_tasks_created_by_other_store() {
    let tmp = tempfile::tempdir().unwrap();
    let store_a = TaskStore::with_dir(tmp.path());
    let store_b = TaskStore::with_dir(tmp.path());
    let task = store_a.create("external", "");

    let listed = store_b.list();

    assert!(listed.iter().any(|entry| entry.id == task.id));
}

#[test]
fn phase0_gap_task_v2_schema_requires_active_form_and_metadata() {
    let create_schema = TaskCreateTool.input_json_schema();
    let create_props = &create_schema["properties"];
    assert!(create_props.get("activeForm").is_some());
    assert!(create_props.get("metadata").is_some());

    let update_schema = TaskUpdateTool.input_json_schema();
    let update_props = &update_schema["properties"];
    for field in [
        "subject",
        "description",
        "activeForm",
        "addBlocks",
        "addBlockedBy",
        "metadata",
    ] {
        assert!(
            update_props.get(field).is_some(),
            "TaskUpdate schema should expose {field}"
        );
    }
    assert!(update_props["status"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str() == Some("deleted")));
}

#[tokio::test]
#[serial_test::serial]
async fn task_create_and_update_persist_active_form_and_metadata() {
    let home = tempfile::tempdir().unwrap();
    let _home = EnvGuard::set("CC_RUST_HOME", home.path());
    let list_id = format!("phase3-schema-{}", uuid::Uuid::new_v4());
    let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_id);
    let ctx = test_context();
    let parent = dummy_parent();

    let created = TaskCreateTool
        .call(
            json!({
                "subject": "original",
                "description": "before",
                "activeForm": "Running original",
                "metadata": {
                    "keep": 1,
                    "drop": true
                }
            }),
            &ctx,
            &parent,
            None,
        )
        .await
        .expect("create task");
    let id = created.data["task"]["id"].as_str().unwrap().to_string();

    let update = TaskUpdateTool
        .call(
            json!({
                "task_id": id,
                "subject": "renamed",
                "description": "after",
                "activeForm": "Running renamed",
                "metadata": {
                    "drop": null,
                    "add": 2
                }
            }),
            &ctx,
            &parent,
            None,
        )
        .await
        .expect("update task");
    assert_eq!(update.data["task"]["subject"], "renamed");
    assert_eq!(update.data["task"]["activeForm"], "Running renamed");

    let restarted = TaskStore::with_dir(task_list_dir(&list_id));
    let persisted = restarted.get(&id).unwrap();
    assert_eq!(persisted.subject, "renamed");
    assert_eq!(persisted.description, "after");
    assert_eq!(persisted.active_form.as_deref(), Some("Running renamed"));
    let metadata = persisted.metadata.unwrap();
    assert_eq!(metadata["keep"], 1);
    assert_eq!(metadata["add"], 2);
    assert!(metadata.get("drop").is_none());
}

#[tokio::test]
#[serial_test::serial]
async fn task_update_adds_dependency_edges_and_deleted_cleans_them() {
    let home = tempfile::tempdir().unwrap();
    let _home = EnvGuard::set("CC_RUST_HOME", home.path());
    let list_id = format!("phase3-deps-{}", uuid::Uuid::new_v4());
    let _list = EnvGuard::set(CC_RUST_TASK_LIST_ID_ENV, &list_id);
    let ctx = test_context();
    let parent = dummy_parent();
    let task_store = store_for_context(&ctx);
    let source = task_store.create("source", "");
    let blocked = task_store.create("blocked", "");
    let blocker = task_store.create("blocker", "");

    TaskUpdateTool
        .call(
            json!({
                "task_id": source.id,
                "addBlocks": [blocked.id],
                "addBlockedBy": [blocker.id]
            }),
            &ctx,
            &parent,
            None,
        )
        .await
        .expect("update dependencies");

    let reloaded = task_store.clone();
    let source_after = reloaded.get(&source.id).unwrap();
    let blocked_after = reloaded.get(&blocked.id).unwrap();
    assert!(source_after.depends_on.iter().any(|id| id == &blocker.id));
    assert!(blocked_after.depends_on.iter().any(|id| id == &source.id));

    let deleted = TaskUpdateTool
        .call(
            json!({
                "taskId": blocker.id,
                "status": "deleted"
            }),
            &ctx,
            &parent,
            None,
        )
        .await
        .expect("delete blocker");
    assert_eq!(deleted.data["success"], true);

    let after_delete = task_store.clone();
    assert!(after_delete.get(&blocker.id).is_none());
    let source_after_delete = after_delete.get(&source.id).unwrap();
    assert!(!source_after_delete
        .depends_on
        .iter()
        .any(|id| id == &blocker.id));
}

#[test]
#[serial_test::serial]
fn phase0_gap_teammate_unassign_is_not_wired() {
    let home = tempfile::tempdir().unwrap();
    let _home = EnvGuard::set("CC_RUST_HOME", home.path());
    let list_id = format!("phase4-unassign-{}", uuid::Uuid::new_v4());
    let teammate_id = "worker@phase4";
    let teammate_name = "worker";
    let store = task_store_for_task_list_id(&list_id);
    let by_id = store.create_with_options(
        "owned by id",
        "",
        TaskCreateOptions {
            owner: Some(teammate_id.to_string()),
            ..TaskCreateOptions::default()
        },
    );
    let by_name = store.create_with_options(
        "owned by name",
        "",
        TaskCreateOptions {
            owner: Some(teammate_name.to_string()),
            ..TaskCreateOptions::default()
        },
    );
    let completed = store.create_with_options(
        "completed stays assigned",
        "",
        TaskCreateOptions {
            owner: Some(teammate_id.to_string()),
            ..TaskCreateOptions::default()
        },
    );
    store.update_status(&by_id.id, TaskStatus::InProgress);
    store.update_status(&by_name.id, TaskStatus::InProgress);
    store.update_status(&completed.id, TaskStatus::Completed);

    let result = unassign_teammate_tasks(
        &list_id,
        teammate_id,
        teammate_name,
        TeammateTaskExitReason::Terminated,
    );

    assert_eq!(result.unassigned_tasks.len(), 2);
    assert!(result.notification_message.contains("was terminated"));
    assert!(result.notification_message.contains(&by_id.id));
    for id in [&by_id.id, &by_name.id] {
        let task = store.get(id).unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.owner, None);
    }
    let completed = store.get(&completed.id).unwrap();
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(completed.owner.as_deref(), Some(teammate_id));

    let repeated = unassign_teammate_tasks(
        &list_id,
        teammate_id,
        teammate_name,
        TeammateTaskExitReason::Shutdown,
    );
    assert!(repeated.unassigned_tasks.is_empty());
}

#[tokio::test]
async fn test_task_output_treats_recoverable_as_active_wait_state() {
    let (_tmp, store) = temp_store();
    let task = store.create("remote", "");
    store.update_status(&task.id, TaskStatus::Recoverable);
    let (_tx, rx) = tokio::sync::watch::channel(false);

    let result = wait_for_task_output(store, &task.id, 0, rx).await.unwrap();
    match result {
        TaskOutputWaitResult::TimedOut(Some(entry)) => {
            assert_eq!(entry.status, TaskStatus::Recoverable);
        }
        other => panic!("expected timeout with recoverable task, got {other:?}"),
    }
}

#[tokio::test]
async fn test_wait_for_task_output_times_out_active_task() {
    let (_tmp, store) = temp_store();
    let task = store.create("running", "");
    store.update_status(&task.id, TaskStatus::InProgress);
    let (_tx, rx) = tokio::sync::watch::channel(false);

    let result = wait_for_task_output(store, &task.id, 0, rx).await.unwrap();
    match result {
        TaskOutputWaitResult::TimedOut(Some(entry)) => {
            assert_eq!(entry.status, TaskStatus::InProgress);
        }
        other => panic!("expected timeout with current task, got {other:?}"),
    }
}

#[tokio::test]
async fn test_wait_for_task_output_observes_completion() {
    let (_tmp, store) = temp_store();
    let task = store.create("running", "");
    store.update_status(&task.id, TaskStatus::InProgress);
    let writer = store.clone();
    let task_id = task.id.clone();
    tokio::spawn(async move {
        sleep(Duration::from_millis(20)).await;
        writer.append_output(&task_id, "done");
        writer.update_status(&task_id, TaskStatus::Completed);
    });
    let (_tx, rx) = tokio::sync::watch::channel(false);

    let result = wait_for_task_output(store, &task.id, 1_000, rx)
        .await
        .unwrap();
    match result {
        TaskOutputWaitResult::Ready(entry) => {
            assert_eq!(entry.status, TaskStatus::Completed);
            assert_eq!(entry.output, "done");
        }
        other => panic!("expected completed task, got {other:?}"),
    }
}

#[test]
fn test_task_to_json() {
    let (_tmp, store) = temp_store();
    let task = store.create("JSON test", "desc");
    let json = task_to_json_for_store(&store, &task);
    assert_eq!(json["subject"], "JSON test");
    assert_eq!(json["description"], "desc");
    assert_eq!(json["status"], "pending");
    assert_eq!(json["kind"], "tool");
    assert_eq!(json["output_truncated"], false);
}

#[test]
fn test_legacy_task_record_migrates() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "legacy-task";
    fs::write(
        tmp.path().join("legacy-task.json"),
        serde_json::to_string_pretty(&json!({
            "id": id,
            "subject": "legacy",
            "description": "old format",
            "status": "stopped",
            "output": "legacy output",
            "created_at": 1,
            "updated_at": 2
        }))
        .unwrap(),
    )
    .unwrap();

    let store = TaskStore::with_dir(tmp.path());
    let migrated = store.get(id).unwrap();
    assert_eq!(migrated.status, TaskStatus::Cancelled);
    assert_eq!(migrated.output, "legacy output");

    let raw = fs::read_to_string(tmp.path().join("legacy-task.json")).unwrap();
    let persisted: PersistedTaskFile = serde_json::from_str(&raw).unwrap();
    assert_eq!(persisted.schema_version, TASK_SCHEMA_VERSION);
    assert_eq!(persisted.task.status, "cancelled");
}

#[test]
fn test_concurrent_output_appends_remain_bounded() {
    let (_tmp, store) = temp_store_with_limit(256);
    let task = store.create("concurrent", "");
    let store = Arc::new(store);
    let mut threads = Vec::new();

    for i in 0..8 {
        let store = store.clone();
        let id = task.id.clone();
        threads.push(thread::spawn(move || {
            for j in 0..25 {
                store.append_output(&id, &format!("line-{i}-{j}"));
            }
        }));
    }

    for thread in threads {
        thread.join().unwrap();
    }

    let entry = store.get(&task.id).unwrap();
    assert!(entry.output.len() <= 256);
    assert!(entry.output_truncated);
}

fn task_to_json_for_store(store: &TaskStore, entry: &TaskEntry) -> Value {
    let blocked_dependencies = store.blocked_dependencies(entry);
    let blocked_tasks = store.blocked_tasks(entry);
    let blocked_by = entry.depends_on.clone();
    json!({
        "id": entry.id,
        "kind": entry.kind,
        "subject": entry.subject,
        "description": entry.description,
        "status": entry.status.as_str(),
        "created_at": entry.created_at,
        "updated_at": entry.updated_at,
        "parent_id": entry.parent_id,
        "depends_on": blocked_by.clone(),
        "blocked_by": blocked_by.clone(),
        "blockedBy": blocked_by,
        "blocks": blocked_tasks,
        "owner": entry.owner,
        "tool_use_id": entry.tool_use_id,
        "agent_id": entry.agent_id,
        "supervisor_id": entry.supervisor_id,
        "isolation": entry.isolation,
        "worktree_path": entry.worktree_path,
        "worktree_branch": entry.worktree_branch,
        "remote_task_type": entry.remote_task_type,
        "remote_session_id": entry.remote_session_id,
        "remote_task_metadata": entry.remote_task_metadata,
        "poll_started_at": entry.poll_started_at,
        "blocked_dependencies": blocked_dependencies,
        "output_summary": entry.output_summary,
        "output_bytes": entry.output_bytes,
        "output_truncated": entry.output_truncated,
        "cancel_requested_at": entry.cancel_requested_at,
        "recovered_at": entry.recovered_at,
        "previous_status": entry.previous_status.map(|s| s.as_str()),
        "has_runtime_handle": store.has_runtime_handle(&entry.id),
    })
}

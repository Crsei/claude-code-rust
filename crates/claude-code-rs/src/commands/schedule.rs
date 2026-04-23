//! `/schedule` — raw management surface for the local cron scheduler
//! (issue #60).
//!
//! ## Scope of the first milestone
//!
//! `/schedule` is explicitly split in two capability lines:
//!
//! - **local cron** — persisted under `{data_root}/scheduled_tasks.json`
//!   and served by [`crate::services::scheduler::SchedulerStore`]. All
//!   subcommands below operate on this store.
//! - **remote triggers** — delegated to a cloud-side agent runtime in the
//!   Bun reference. In cc-rust the remote path requires OAuth/API
//!   groundwork we haven't landed yet, so `/schedule remote …` currently
//!   refuses and points the user at the design doc.
//!
//! Keeping the two lines syntactically distinct means the first milestone
//! ships without implying the second works. When the remote path lands,
//! it'll extend the `remote` subcommand without changing the local
//! surface.
//!
//! ## Subcommands
//!
//!   /schedule                    alias for 'list'
//!   /schedule list               list all scheduled tasks
//!   /schedule add <interval> <payload>   add a new local task (no immediate run)
//!   /schedule show <id>          inspect one task
//!   /schedule remove <id>        delete a task
//!   /schedule pause <id>         suspend a task without deleting
//!   /schedule resume <id>        re-enable a paused task
//!   /schedule trigger <id>       mark task as fired and roll next_run_at forward
//!   /schedule remote …           (disabled) surface for remote triggers

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::services::scheduler::{
    parse_interval, Interval, ScheduledTask, SchedulerError, SchedulerKind, SchedulerStore, TaskId,
    TaskPayload,
};

pub struct ScheduleHandler;

#[async_trait]
impl CommandHandler for ScheduleHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let store = SchedulerStore::open_default();
        Ok(CommandResult::Output(dispatch(&store, args)))
    }
}

fn dispatch(store: &SchedulerStore, args: &str) -> String {
    let trimmed = args.trim();
    let (head, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((h, r)) => (h.to_lowercase(), r.trim()),
        None => (trimmed.to_lowercase(), ""),
    };

    match head.as_str() {
        "" | "list" | "ls" => list(store),
        "help" | "--help" | "-h" => help_text(store),
        "add" | "create" | "new" => add(store, rest),
        "show" | "info" => show(store, rest),
        "remove" | "rm" | "delete" => remove(store, rest),
        "pause" => set_paused(store, rest, true),
        "resume" | "unpause" => set_paused(store, rest, false),
        "trigger" | "fire" => trigger(store, rest),
        "due" => due(store),
        "remote" => remote_hint(rest),
        other => format!(
            "Unknown /schedule subcommand '{}'. Run '/schedule help' for the list.",
            other
        ),
    }
}

fn help_text(store: &SchedulerStore) -> String {
    let storage = store.path().display().to_string();
    [
        "/schedule — local cron scheduler.".to_string(),
        String::new(),
        "Scope: this command manages LOCAL cron tasks. Remote triggers".to_string(),
        "  (cloud-side scheduled agents) are tracked as a separate".to_string(),
        "  capability line and are not yet implemented — see".to_string(),
        "  `/schedule remote`.".to_string(),
        format!("  Storage: {}", storage),
        String::new(),
        "  /schedule                    list all scheduled tasks".to_string(),
        "  /schedule add <interval> <payload>   add a new local task".to_string(),
        "  /schedule show <id>          inspect one task".to_string(),
        "  /schedule remove <id>        delete a task".to_string(),
        "  /schedule pause <id>         suspend a task".to_string(),
        "  /schedule resume <id>        re-enable a paused task".to_string(),
        "  /schedule trigger <id>       mark task as fired now".to_string(),
        "  /schedule due                show tasks that are due right now".to_string(),
        "  /schedule remote …           (disabled) remote-trigger surface".to_string(),
        String::new(),
        "Interval examples: 30s, 5m, 1h, 2d, '*/10 * * * *'.".to_string(),
        "Payload is a slash command (/simplify) or a plain prompt.".to_string(),
        String::new(),
        "Tip: /loop is a higher-level wrapper that also runs the payload".to_string(),
        "once immediately.".to_string(),
    ]
    .join("\n")
}

fn due(store: &SchedulerStore) -> String {
    match store.due_tasks() {
        Ok(tasks) => {
            if tasks.is_empty() {
                return "No scheduled tasks are due right now.".into();
            }
            let mut out = format!("{} task(s) due now\n", tasks.len());
            out.push_str(&"─".repeat(24));
            out.push('\n');
            for t in &tasks {
                out.push_str(&render_row(t));
            }
            out.push_str(
                "\nTip: the daemon tick loop (if running) will execute these \
                 on its next poll; use `/schedule trigger <id>` to mark as fired.\n",
            );
            out
        }
        Err(e) => format!("Could not read scheduler state: {}", e),
    }
}

fn list(store: &SchedulerStore) -> String {
    match store.load() {
        Ok(tasks) => {
            if tasks.is_empty() {
                return "No scheduled tasks registered. Try '/schedule add <interval> <payload>'."
                    .into();
            }
            let mut out = String::new();
            out.push_str(&format!("Local scheduled tasks ({})\n", tasks.len()));
            out.push_str(&"─".repeat(26));
            out.push('\n');
            for t in &tasks {
                out.push_str(&render_row(t));
            }
            out.push_str("\nRemote triggers: (not implemented — see `/schedule remote`).\n");
            out
        }
        Err(e) => format!("Could not read scheduler state: {}", e),
    }
}

fn render_row(t: &ScheduledTask) -> String {
    let status = if t.paused { "paused" } else { "active" };
    format!(
        "  {id}  every {interval}  [{kind} · {status}]\n    \
         payload ({ptype}): {payload}\n    next run: {next}\n",
        id = t.id,
        interval = Interval::from_seconds(t.interval_seconds).human(),
        kind = t.kind.as_str(),
        status = status,
        ptype = t.payload.kind_label(),
        payload = t.payload.display(),
        next = t.next_run_at.to_rfc3339(),
    )
}

fn add(store: &SchedulerStore, rest: &str) -> String {
    let Some((interval_raw, payload_raw)) = rest.split_once(char::is_whitespace) else {
        return "Usage: /schedule add <interval> <payload>".into();
    };
    let payload_raw = payload_raw.trim();
    if payload_raw.is_empty() {
        return "Usage: /schedule add <interval> <payload>".into();
    }

    let interval = match parse_interval(interval_raw) {
        Ok(i) => i,
        Err(e) => return format!("Could not parse interval '{}': {}", interval_raw, e),
    };

    let payload = TaskPayload::from_user_input(payload_raw);
    let task = ScheduledTask::new(
        SchedulerKind::LocalCron,
        derive_name(&payload),
        interval_raw,
        interval,
        payload,
        Utc::now(),
    );

    match store.add(task) {
        Ok(saved) => format!(
            "Added scheduled task '{}' (id={}) — runs every {} starting at {}.",
            saved.name,
            saved.id,
            interval.human(),
            saved.next_run_at.to_rfc3339()
        ),
        Err(e) => format!("Could not add task: {}", e),
    }
}

fn show(store: &SchedulerStore, id_raw: &str) -> String {
    if id_raw.is_empty() {
        return "Usage: /schedule show <id>".into();
    }
    let id = TaskId(id_raw.to_string());
    match store.get(&id) {
        Ok(t) => {
            let mut out = format!("Scheduled task {}\n", t.id);
            out.push_str(&"─".repeat(16 + t.id.as_str().len()));
            out.push('\n');
            out.push_str(&format!("  Name:          {}\n", t.name));
            out.push_str(&format!("  Kind:          {}\n", t.kind.as_str()));
            out.push_str(&format!(
                "  Status:        {}\n",
                if t.paused { "paused" } else { "active" }
            ));
            out.push_str(&format!(
                "  Interval:      {} (raw: {})\n",
                Interval::from_seconds(t.interval_seconds).human(),
                t.schedule
            ));
            out.push_str(&format!("  Payload kind:  {}\n", t.payload.kind_label()));
            out.push_str(&format!("  Payload:       {}\n", t.payload.display()));
            out.push_str(&format!("  Created at:    {}\n", t.created_at.to_rfc3339()));
            out.push_str(&format!(
                "  Last run at:   {}\n",
                t.last_run_at
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_else(|| "never".to_string())
            ));
            out.push_str(&format!(
                "  Next run at:   {}\n",
                t.next_run_at.to_rfc3339()
            ));
            out
        }
        Err(SchedulerError::NotFound(_)) => {
            format!("No scheduled task with id '{}'.", id_raw)
        }
        Err(e) => format!("Could not read task: {}", e),
    }
}

fn remove(store: &SchedulerStore, id_raw: &str) -> String {
    if id_raw.is_empty() {
        return "Usage: /schedule remove <id>".into();
    }
    let id = TaskId(id_raw.to_string());
    match store.remove(&id) {
        Ok(removed) => format!(
            "Removed scheduled task '{}' (id={}).",
            removed.name, removed.id
        ),
        Err(SchedulerError::NotFound(_)) => {
            format!("No scheduled task with id '{}'.", id_raw)
        }
        Err(e) => format!("Could not remove task: {}", e),
    }
}

fn set_paused(store: &SchedulerStore, id_raw: &str, paused: bool) -> String {
    if id_raw.is_empty() {
        return if paused {
            "Usage: /schedule pause <id>".into()
        } else {
            "Usage: /schedule resume <id>".into()
        };
    }
    let id = TaskId(id_raw.to_string());
    match store.set_paused(&id, paused) {
        Ok(task) => {
            let verb = if paused { "Paused" } else { "Resumed" };
            format!("{} scheduled task '{}' (id={}).", verb, task.name, task.id)
        }
        Err(SchedulerError::NotFound(_)) => {
            format!("No scheduled task with id '{}'.", id_raw)
        }
        Err(e) => format!("Could not update task: {}", e),
    }
}

fn trigger(store: &SchedulerStore, id_raw: &str) -> String {
    if id_raw.is_empty() {
        return "Usage: /schedule trigger <id>".into();
    }
    let id = TaskId(id_raw.to_string());
    match store.record_fired(&id) {
        Ok(task) => format!(
            "Marked task '{}' (id={}) as fired. Next run at {}.",
            task.name, task.id, task.next_run_at
        ),
        Err(SchedulerError::NotFound(_)) => {
            format!("No scheduled task with id '{}'.", id_raw)
        }
        Err(e) => format!("Could not trigger task: {}", e),
    }
}

fn remote_hint(_rest: &str) -> String {
    [
        "Remote triggers are not implemented yet in cc-rust (issue #60).",
        "",
        "The first /schedule milestone covers LOCAL cron only — tasks persist",
        "to {data_root}/scheduled_tasks.json and are run by the current",
        "process. The remote-trigger capability requires cloud OAuth and",
        "agent APIs that haven't been ported from the Bun reference yet.",
        "",
        "Use '/schedule list' to see local tasks.",
    ]
    .join("\n")
}

fn derive_name(payload: &TaskPayload) -> String {
    let raw = payload.display();
    let first_line = raw.lines().next().unwrap_or(raw).trim();
    let short: String = first_line.chars().take(40).collect();
    if short.is_empty() {
        "task".into()
    } else {
        short
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn fresh_store() -> (tempfile::TempDir, SchedulerStore) {
        let dir = tempdir().unwrap();
        let store = SchedulerStore::new(dir.path().join("scheduled_tasks.json"));
        (dir, store)
    }

    #[test]
    fn empty_args_lists_tasks() {
        let (_dir, store) = fresh_store();
        let out = dispatch(&store, "");
        assert!(out.contains("No scheduled tasks"));
    }

    #[test]
    fn help_documents_scope() {
        let (_dir, store) = fresh_store();
        let help = help_text(&store);
        assert!(help.contains("local cron"));
        assert!(help.contains("remote triggers") || help.contains("Remote triggers"));
        assert!(help.contains("not yet implemented") || help.contains("disabled"));
        assert!(
            help.contains(store.path().display().to_string().as_str()),
            "help output should include the storage path"
        );
    }

    #[test]
    fn due_lists_only_tasks_past_next_run() {
        let (_dir, store) = fresh_store();
        // Task scheduled to run in the future — should NOT be due.
        dispatch(&store, "add 1h future thing");
        assert!(dispatch(&store, "due").contains("No scheduled tasks are due"));

        // Mutate the stored task so its next_run_at is in the past.
        let mut tasks = store.load().unwrap();
        tasks[0].next_run_at = Utc::now() - chrono::Duration::seconds(30);
        store.remove(&tasks[0].id).unwrap();
        store
            .add(ScheduledTask {
                id: tasks[0].id.clone(),
                ..tasks.remove(0)
            })
            .unwrap();

        let out = dispatch(&store, "due");
        assert!(
            out.contains("task(s) due now"),
            "unexpected due output: {}",
            out
        );
        assert!(out.contains("future thing"));
    }

    #[test]
    fn add_then_list_shows_task() {
        let (_dir, store) = fresh_store();
        let out = dispatch(&store, "add 5m run the deploy check");
        assert!(out.contains("Added scheduled task"));
        let list_out = dispatch(&store, "list");
        assert!(list_out.contains("run the deploy check"));
        assert!(list_out.contains("every 5m"));
    }

    #[test]
    fn show_inspects_task() {
        let (_dir, store) = fresh_store();
        dispatch(&store, "add 1h do the thing");
        let id = store.load().unwrap()[0].id.clone();
        let out = dispatch(&store, &format!("show {}", id));
        assert!(out.contains("Payload:"));
        assert!(out.contains("do the thing"));
        assert!(out.contains("Interval:"));
        assert!(out.contains("Next run at:"));
    }

    #[test]
    fn remove_and_pause_and_resume() {
        let (_dir, store) = fresh_store();
        dispatch(&store, "add 30s keep watching");
        let id = store.load().unwrap()[0].id.clone();

        assert!(dispatch(&store, &format!("pause {}", id)).contains("Paused"));
        assert!(store.load().unwrap()[0].paused);
        assert!(dispatch(&store, &format!("resume {}", id)).contains("Resumed"));
        assert!(!store.load().unwrap()[0].paused);
        assert!(dispatch(&store, &format!("remove {}", id)).contains("Removed"));
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn trigger_advances_next_run() {
        let (_dir, store) = fresh_store();
        dispatch(&store, "add 5m keep going");
        let id = store.load().unwrap()[0].id.clone();
        let before = store.get(&id).unwrap().next_run_at;
        let out = dispatch(&store, &format!("trigger {}", id));
        assert!(out.contains("Marked task"));
        let after = store.get(&id).unwrap().next_run_at;
        assert!(after >= before);
    }

    #[test]
    fn remote_subcommand_refuses_with_context() {
        let (_dir, store) = fresh_store();
        let out = dispatch(&store, "remote add 1h foo");
        assert!(out.contains("not implemented"));
        // We want the refusal to explain what IS implemented — case
        // insensitively, since the user copy uses "LOCAL cron" for emphasis.
        assert!(out.to_lowercase().contains("local cron"));
    }

    #[test]
    fn unknown_subcommand_reports_error() {
        let (_dir, store) = fresh_store();
        let out = dispatch(&store, "frobnicate");
        assert!(out.contains("Unknown /schedule subcommand"));
    }

    #[test]
    fn add_without_payload_shows_usage() {
        let (_dir, store) = fresh_store();
        assert!(dispatch(&store, "add 5m").contains("Usage"));
        assert!(dispatch(&store, "add").contains("Usage"));
    }
}

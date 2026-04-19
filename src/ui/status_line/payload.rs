//! JSON payload schema for the scriptable status line.
//!
//! Corresponds to `docs/claude-code-configuration/customize-status-line.md`.
//! The runtime assembles a [`StatusLinePayload`] once per refresh and pipes
//! it to the user's `statusLine.command` on stdin.
//!
//! The shape is intentionally stable / documented so shell/python scripts
//! can rely on it. Every nested struct uses `serde(skip_serializing_if)` for
//! `Option`s and empty collections so absent fields really are absent from
//! the JSON (scripts can `.get()` with confidence).

use serde::{Deserialize, Serialize};

/// Top-level payload piped to the user's status-line command on stdin.
///
/// All fields except `hook_event_name` and `model` are optional —
/// missing data (e.g. no active worktree, no cost yet) is represented by
/// the field being absent rather than a zero value, so scripts can tell
/// "unknown" apart from "known to be zero".
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StatusLinePayload {
    /// Static event name so scripts can multiplex if the payload is ever
    /// reused for other hooks. Always `"StatusLine"`.
    pub hook_event_name: String,

    /// Protocol version — bump when fields are removed or renamed so
    /// scripts can fail fast. Additive changes keep the same version.
    pub version: u32,

    /// Session identifier (UUID-like string from the engine).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Active model (resolved main-loop model).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelInfo>,

    /// Workspace / cwd information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceStatus>,

    /// Context-window occupancy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ContextWindowStatus>,

    /// Cost accumulator for the current session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostStatus>,

    /// Current output style, e.g. `"default"` / `"explanatory"` / `"learning"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_style: Option<String>,

    /// Vim mode information (only present when `editorMode` is `vim`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vim: Option<VimStatus>,

    /// Currently streaming an assistant response?
    pub streaming: bool,

    /// Integer count of on-screen messages (user + assistant + tool).
    pub message_count: usize,
}

/// Model identity exposed to the status-line script.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Full model ID (e.g. `claude-sonnet-4-20250514`).
    pub id: String,
    /// Short display form (e.g. `sonnet-4`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Backend routing label (e.g. `native`, `codex`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
}

/// Workspace / cwd snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    /// Current working directory (absolute path).
    pub cwd: String,
    /// Project root (e.g. git toplevel) when distinct from `cwd`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_dir: Option<String>,
    /// Active git branch, when in a repo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_branch: Option<String>,
    /// True when the checkout is a git worktree (not the main repo).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_worktree: Option<bool>,
}

/// Context-window occupancy — matches the `/context` command output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ContextWindowStatus {
    /// Input tokens consumed so far in this session.
    pub input_tokens: u64,
    /// Output tokens produced so far.
    pub output_tokens: u64,
    /// Cache-read tokens (billed at a discount).
    pub cache_read_tokens: u64,
    /// Cache-creation tokens.
    pub cache_creation_tokens: u64,
    /// Total context window ceiling, when known (e.g. 200_000 for Claude 3.5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    /// `input_tokens / max_tokens` as a 0..=1 fraction, when `max_tokens` is
    /// known. Rounded to 4 decimals.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_fraction: Option<f64>,
}

/// Session-wide cost tracking.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CostStatus {
    /// Accumulated USD cost.
    pub total_usd: f64,
    /// Number of API calls made.
    pub api_calls: u64,
    /// Wall-clock duration of the current session, in seconds. Optional so
    /// scripts that don't care about timing can ignore it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_duration_secs: Option<u64>,
}

/// Vim editor status.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct VimStatus {
    /// One of `"normal"`, `"insert"`, `"visual"`, `"command"`.
    pub mode: String,
}

impl StatusLinePayload {
    /// Protocol version. Bump on breaking changes.
    pub const VERSION: u32 = 1;

    /// Construct a minimal payload with just the required bookkeeping
    /// fields filled in. Callers then set fields from state snapshots.
    pub fn new() -> Self {
        Self {
            hook_event_name: "StatusLine".to_string(),
            version: Self::VERSION,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_serializes_minimum_required_keys() {
        let payload = StatusLinePayload::new();
        let v = serde_json::to_value(&payload).unwrap();
        assert_eq!(
            v.get("hookEventName").and_then(|x| x.as_str()),
            Some("StatusLine")
        );
        assert_eq!(v.get("version").and_then(|x| x.as_u64()), Some(1));
        assert_eq!(v.get("streaming").and_then(|x| x.as_bool()), Some(false));
        // Absent-optional fields really are absent from the JSON.
        assert!(v.get("sessionId").is_none());
        assert!(v.get("model").is_none());
        assert!(v.get("workspace").is_none());
    }

    #[test]
    fn payload_roundtrips_with_nested_structs() {
        let p = StatusLinePayload {
            hook_event_name: "StatusLine".to_string(),
            version: 1,
            session_id: Some("abc-123".into()),
            model: Some(ModelInfo {
                id: "claude-sonnet-4-20250514".into(),
                display_name: Some("sonnet-4".into()),
                backend: Some("native".into()),
            }),
            workspace: Some(WorkspaceStatus {
                cwd: "/tmp/x".into(),
                project_dir: Some("/tmp/x".into()),
                git_branch: Some("main".into()),
                is_worktree: Some(false),
            }),
            context: Some(ContextWindowStatus {
                input_tokens: 1000,
                output_tokens: 500,
                cache_read_tokens: 2000,
                cache_creation_tokens: 0,
                max_tokens: Some(200_000),
                used_fraction: Some(0.005),
            }),
            cost: Some(CostStatus {
                total_usd: 0.0123,
                api_calls: 3,
                session_duration_secs: Some(42),
            }),
            output_style: Some("default".into()),
            vim: Some(VimStatus {
                mode: "normal".into(),
            }),
            streaming: true,
            message_count: 7,
        };
        let s = serde_json::to_string(&p).unwrap();
        let parsed: StatusLinePayload = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.session_id.as_deref(), Some("abc-123"));
        assert_eq!(
            parsed.model.as_ref().unwrap().id,
            "claude-sonnet-4-20250514"
        );
        assert_eq!(parsed.context.as_ref().unwrap().input_tokens, 1000);
        assert!(parsed.streaming);
        assert_eq!(parsed.message_count, 7);
    }

    #[test]
    fn new_fills_event_name_and_version() {
        let p = StatusLinePayload::new();
        assert_eq!(p.hook_event_name, "StatusLine");
        assert_eq!(p.version, StatusLinePayload::VERSION);
        assert!(!p.streaming);
        assert_eq!(p.message_count, 0);
    }
}

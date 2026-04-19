//! Session storage -- persisting conversation state to disk.
//!
//! Sessions are stored as JSON files under `~/.cc-rust/sessions/`.
//! Each session is identified by a UUID and contains the full message history
//! along with metadata (creation time, working directory, etc.).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::Utc;
use git2::Repository;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::types::message::Message;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Metadata about a saved session, returned by `list_sessions`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Unique session identifier (UUID v4).
    pub session_id: String,
    /// Unix timestamp (seconds) when the session was created.
    pub created_at: i64,
    /// Unix timestamp (seconds) of the last modification.
    pub last_modified: i64,
    /// Number of messages in the session.
    pub message_count: usize,
    /// Working directory at the time the session was created.
    pub cwd: String,
}

/// On-disk representation of a saved session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFile {
    pub session_id: String,
    pub created_at: i64,
    pub last_modified: i64,
    pub cwd: String,
    pub messages: Vec<SerializableMessage>,
}

/// Simplified serializable message wrapper.
///
/// `Message` itself is a complex enum. For persistence we flatten it into a
/// tagged JSON representation. The real implementation would use a custom
/// Serialize/Deserialize impl on `Message`; for now we store the JSON value
/// directly so we don't lose data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub uuid: String,
    pub timestamp: i64,
    pub data: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Return the base directory for session storage. Resolves through
/// [`crate::config::paths::sessions_dir`].
pub fn get_session_dir() -> PathBuf {
    crate::config::paths::sessions_dir()
}

/// Return the file path for a specific session.
pub fn get_session_file(session_id: &str) -> PathBuf {
    get_session_dir().join(format!("{}.json", session_id))
}

fn normalize_display_path(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.components().collect())
        .to_string_lossy()
        .to_string()
}

fn stable_workspace_path(path: &Path) -> PathBuf {
    crate::utils::git::find_git_root(path).unwrap_or_else(|| path.to_path_buf())
}

fn normalize_match_key(path: &Path) -> String {
    let normalized: PathBuf =
        std::fs::canonicalize(path).unwrap_or_else(|_| path.components().collect());
    let mut value = normalized.to_string_lossy().to_string();

    if cfg!(windows) {
        value = value.replace('/', "\\").to_lowercase();
    }

    value
}

fn git_common_dir(repo: &Repository) -> PathBuf {
    if repo.is_worktree() {
        repo.path()
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| repo.path().to_path_buf())
    } else {
        repo.path().to_path_buf()
    }
}

fn workspace_key(path: &Path) -> String {
    if let Ok(repo) = Repository::discover(path) {
        return normalize_match_key(&git_common_dir(&repo));
    }

    normalize_match_key(&stable_workspace_path(path))
}

fn filter_sessions_for_workspace(mut sessions: Vec<SessionInfo>, cwd: &Path) -> Vec<SessionInfo> {
    let current_workspace = workspace_key(cwd);
    sessions.retain(|session| workspace_key(Path::new(&session.cwd)) == current_workspace);
    sessions
}

// ---------------------------------------------------------------------------
// Persistence operations
// ---------------------------------------------------------------------------

/// Save a session to disk.
///
/// Creates the sessions directory if it does not exist. Overwrites any
/// existing file for the same `session_id`.
pub fn save_session(session_id: &str, messages: &[Message], cwd: &str) -> Result<()> {
    let dir = get_session_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create session directory {}", dir.display()))?;

    let path = get_session_file(session_id);

    let now = Utc::now().timestamp();

    // Try to preserve the original created_at if we are updating.
    let created_at = if path.exists() {
        load_session_file(session_id)
            .map(|f| f.created_at)
            .unwrap_or(now)
    } else {
        now
    };

    let serializable_messages = messages_to_serializable(messages);
    let msg_count = serializable_messages.len();

    let stable_cwd = normalize_display_path(&stable_workspace_path(Path::new(cwd)));

    let session_file = SessionFile {
        session_id: session_id.to_string(),
        created_at,
        last_modified: now,
        cwd: stable_cwd,
        messages: serializable_messages,
    };

    let json =
        serde_json::to_string_pretty(&session_file).context("Failed to serialize session")?;

    std::fs::write(&path, json)
        .with_context(|| format!("Failed to write session file {}", path.display()))?;

    debug!(
        session_id = session_id,
        messages = msg_count,
        "session saved"
    );

    Ok(())
}

/// Load a session from disk and return the messages.
pub fn load_session(session_id: &str) -> Result<Vec<Message>> {
    let file = load_session_file(session_id)?;
    let messages = serializable_to_messages(&file.messages);
    debug!(
        session_id = session_id,
        messages = messages.len(),
        "session loaded"
    );
    Ok(messages)
}

/// Load the raw session file.
fn load_session_file(session_id: &str) -> Result<SessionFile> {
    let path = get_session_file(session_id);
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read session file {}", path.display()))?;
    let file: SessionFile = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse session file {}", path.display()))?;
    Ok(file)
}

/// List all available sessions, sorted by last_modified (most recent first).
pub fn list_sessions() -> Result<Vec<SessionInfo>> {
    let dir = get_session_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions: Vec<SessionInfo> = Vec::new();

    for entry in std::fs::read_dir(&dir)
        .with_context(|| format!("Failed to read session directory {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map_or(true, |ext| ext != "json") {
            continue;
        }

        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let file: SessionFile = match serde_json::from_str(&contents) {
            Ok(f) => f,
            Err(_) => continue,
        };

        sessions.push(SessionInfo {
            session_id: file.session_id,
            created_at: file.created_at,
            last_modified: file.last_modified,
            message_count: file.messages.len(),
            cwd: file.cwd,
        });
    }

    // Most recently modified first.
    sessions.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

    debug!(count = sessions.len(), "sessions listed");

    Ok(sessions)
}

/// List sessions that belong to the same workspace/repository as `cwd`.
///
/// For git repositories, this groups together all worktrees that share the
/// same git common directory. For non-git directories, it falls back to the
/// stable workspace path (repo root if inside git, otherwise the exact path).
pub fn list_workspace_sessions(cwd: &Path) -> Result<Vec<SessionInfo>> {
    Ok(filter_sessions_for_workspace(list_sessions()?, cwd))
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Convert the internal `Message` enum to a serializable form.
fn messages_to_serializable(messages: &[Message]) -> Vec<SerializableMessage> {
    messages
        .iter()
        .map(|msg| {
            let (msg_type, data) = match msg {
                Message::User(u) => {
                    let content_value = match &u.content {
                        crate::types::message::MessageContent::Text(t) => {
                            serde_json::json!(t)
                        }
                        crate::types::message::MessageContent::Blocks(blocks) => {
                            serde_json::json!(blocks)
                        }
                    };
                    (
                        "user".to_string(),
                        serde_json::json!({
                            "content": content_value,
                            "is_meta": u.is_meta,
                        }),
                    )
                }
                Message::Assistant(a) => (
                    "assistant".to_string(),
                    serde_json::json!({
                        "content": a.content,
                        "stop_reason": a.stop_reason,
                        "cost_usd": a.cost_usd,
                        "usage": a.usage,
                    }),
                ),
                Message::System(s) => (
                    "system".to_string(),
                    serde_json::json!({
                        "content": s.content,
                    }),
                ),
                Message::Progress(p) => (
                    "progress".to_string(),
                    serde_json::json!({
                        "tool_use_id": p.tool_use_id,
                        "data": p.data,
                    }),
                ),
                Message::Attachment(a) => (
                    "attachment".to_string(),
                    serde_json::json!({
                        "attachment": a.attachment,
                    }),
                ),
            };
            SerializableMessage {
                msg_type,
                uuid: msg.uuid().to_string(),
                timestamp: msg.timestamp(),
                data,
            }
        })
        .collect()
}

/// Convert serializable messages back to `Message` instances.
///
/// This is a best-effort reconstruction. Fields that cannot be recovered from
/// the simplified serialization are set to defaults. A production
/// implementation would store the full typed data.
fn serializable_to_messages(msgs: &[SerializableMessage]) -> Vec<Message> {
    use crate::types::message::*;
    use uuid::Uuid;

    msgs.iter()
        .filter_map(|sm| {
            let uuid = Uuid::parse_str(&sm.uuid).unwrap_or_else(|_| Uuid::new_v4());

            match sm.msg_type.as_str() {
                "user" => Some(Message::User(UserMessage {
                    uuid,
                    timestamp: sm.timestamp,
                    role: "user".into(),
                    content: match sm.data.get("content") {
                        Some(serde_json::Value::String(s)) => MessageContent::Text(s.clone()),
                        Some(serde_json::Value::Array(blocks)) => {
                            match serde_json::from_value::<Vec<crate::types::message::ContentBlock>>(
                                serde_json::Value::Array(blocks.clone()),
                            ) {
                                Ok(cb) => MessageContent::Blocks(cb),
                                Err(_) => MessageContent::Text(
                                    blocks.iter()
                                        .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                                        .collect::<Vec<_>>()
                                        .join("\n"),
                                ),
                            }
                        }
                        // Backwards compat: old Debug format like Text("hello")
                        _ => MessageContent::Text(
                            sm.data.get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                        ),
                    },
                    is_meta: sm
                        .data
                        .get("is_meta")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    tool_use_result: None,
                    source_tool_assistant_uuid: None,
                })),
                "assistant" => Some(Message::Assistant(AssistantMessage {
                    uuid,
                    timestamp: sm.timestamp,
                    role: "assistant".into(),
                    content: sm.data.get("content")
                        .and_then(|v| serde_json::from_value::<Vec<crate::types::message::ContentBlock>>(v.clone()).ok())
                        .unwrap_or_default(),
                    usage: sm.data.get("usage")
                        .and_then(|v| serde_json::from_value::<crate::types::message::Usage>(v.clone()).ok()),
                    stop_reason: sm
                        .data
                        .get("stop_reason")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    is_api_error_message: false,
                    api_error: None,
                    cost_usd: sm
                        .data
                        .get("cost_usd")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0),
                })),
                "system" => Some(Message::System(SystemMessage {
                    uuid,
                    timestamp: sm.timestamp,
                    subtype: SystemSubtype::Informational {
                        level: InfoLevel::Info,
                    },
                    content: sm
                        .data
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })),
                _ => None,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_session_dir_path() {
        let dir = get_session_dir();
        let dir_str = dir.to_string_lossy();
        assert!(dir_str.contains("sessions"));
    }

    #[test]
    fn test_session_file_path() {
        let path = get_session_file("abc-123");
        assert!(path.to_string_lossy().ends_with("abc-123.json"));
    }

    #[test]
    fn test_stable_workspace_path_uses_git_root() {
        let temp = tempdir().unwrap();
        let repo_dir = temp.path().join("repo");
        std::fs::create_dir_all(repo_dir.join("target").join("release")).unwrap();
        Repository::init(&repo_dir).unwrap();

        let nested = repo_dir.join("target").join("release");
        assert_eq!(stable_workspace_path(&nested), repo_dir);
    }

    #[test]
    fn test_filter_sessions_for_workspace_matches_same_repo_subdirs() {
        let temp = tempdir().unwrap();
        let repo_dir = temp.path().join("repo");
        let nested_dir = repo_dir.join("target").join("release");
        let other_dir = temp.path().join("other");

        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::create_dir_all(&other_dir).unwrap();
        Repository::init(&repo_dir).unwrap();

        let sessions = vec![
            SessionInfo {
                session_id: "repo-root".into(),
                created_at: 0,
                last_modified: 3,
                message_count: 10,
                cwd: normalize_display_path(&repo_dir),
            },
            SessionInfo {
                session_id: "repo-nested".into(),
                created_at: 0,
                last_modified: 2,
                message_count: 8,
                cwd: normalize_display_path(&nested_dir),
            },
            SessionInfo {
                session_id: "other".into(),
                created_at: 0,
                last_modified: 1,
                message_count: 2,
                cwd: normalize_display_path(&other_dir),
            },
        ];

        let filtered = filter_sessions_for_workspace(sessions, &nested_dir);
        let ids: Vec<_> = filtered.into_iter().map(|s| s.session_id).collect();
        assert_eq!(ids, vec!["repo-root", "repo-nested"]);
    }

    #[cfg(windows)]
    #[test]
    fn test_workspace_key_is_case_insensitive_on_windows() {
        let temp = tempdir().unwrap();
        let repo_dir = temp.path().join("Repo");
        std::fs::create_dir_all(&repo_dir).unwrap();
        Repository::init(&repo_dir).unwrap();

        let upper = repo_dir.to_string_lossy().to_uppercase().replace('\\', "/");
        let lower = repo_dir.to_string_lossy().to_lowercase();

        assert_eq!(
            workspace_key(Path::new(&upper)),
            workspace_key(Path::new(&lower))
        );
    }
}

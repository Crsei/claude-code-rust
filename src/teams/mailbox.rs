//! File-based mailbox IPC for teammate communication.
//!
//! Corresponds to TypeScript: `utils/teammateMailbox.ts`
//!
//! Each teammate has an inbox file at:
//!   `~/.cc-rust/teams/{team_name}/inboxes/{agent_name}.json`
//!
//! Messages are stored as a JSON array of `TeammateMessage`.
//! Write operations use file locking to prevent data loss from concurrent access.

#![allow(unused)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use tracing::{debug, warn};

use super::constants::*;
use super::types::TeammateMessage;

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Get the base directory for a team's data.
///
/// Returns: `~/.cc-rust/teams/{team_name}`
pub fn team_dir(team_name: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".cc-rust")
        .join(TEAMS_DIR_NAME)
        .join(sanitize_name(team_name))
}

/// Get the inbox file path for an agent.
///
/// Returns: `~/.cc-rust/teams/{team_name}/inboxes/{agent_name}.json`
pub fn inbox_path(agent_name: &str, team_name: &str) -> PathBuf {
    team_dir(team_name)
        .join(INBOXES_DIR_NAME)
        .join(format!("{}.{}", sanitize_name(agent_name), INBOX_EXTENSION))
}

/// Get the lock file path for an inbox.
fn lock_path(inbox: &Path) -> PathBuf {
    let mut p = inbox.as_os_str().to_owned();
    p.push(LOCK_FILE_SUFFIX);
    PathBuf::from(p)
}

/// Sanitize a name for use in file paths (replace problematic characters).
fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect()
}

// ---------------------------------------------------------------------------
// Core operations
// ---------------------------------------------------------------------------

/// Read all messages from a teammate's mailbox.
///
/// No locking — reads are best-effort.
///
/// Corresponds to TS: `readMailbox(name, team)`
pub fn read_mailbox(agent_name: &str, team_name: &str) -> Result<Vec<TeammateMessage>> {
    let path = inbox_path(agent_name, team_name);
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read mailbox: {}", path.display()))?;
    if content.trim().is_empty() {
        return Ok(vec![]);
    }
    let messages: Vec<TeammateMessage> = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse mailbox: {}", path.display()))?;
    Ok(messages)
}

/// Read only unread messages from a teammate's mailbox.
///
/// Corresponds to TS: `readUnreadMessages(name, team)`
pub fn read_unread_messages(agent_name: &str, team_name: &str) -> Result<Vec<TeammateMessage>> {
    let messages = read_mailbox(agent_name, team_name)?;
    Ok(messages.into_iter().filter(|m| !m.read).collect())
}

/// Write a message to a teammate's mailbox.
///
/// Uses file locking to prevent concurrent write corruption.
/// Lock → read latest → append → write back.
///
/// Corresponds to TS: `writeToMailbox(name, msg, team)`
pub fn write_to_mailbox(
    agent_name: &str,
    message: TeammateMessage,
    team_name: &str,
) -> Result<()> {
    let path = inbox_path(agent_name, team_name);

    // Ensure directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    with_lock(&path, || {
        // Re-read to capture concurrent writes
        let mut messages = if path.exists() {
            let content = fs::read_to_string(&path).unwrap_or_else(|_| "[]".into());
            serde_json::from_str::<Vec<TeammateMessage>>(&content).unwrap_or_default()
        } else {
            vec![]
        };

        messages.push(message);

        let json = serde_json::to_string_pretty(&messages)?;
        fs::write(&path, json)?;
        Ok(())
    })
}

/// Mark a specific message as read by index.
///
/// Corresponds to TS: `markAsReadByIndex(name, team, idx)`
pub fn mark_as_read_by_index(
    agent_name: &str,
    team_name: &str,
    index: usize,
) -> Result<()> {
    let path = inbox_path(agent_name, team_name);
    if !path.exists() {
        return Ok(());
    }

    with_lock(&path, || {
        let content = fs::read_to_string(&path)?;
        let mut messages: Vec<TeammateMessage> = serde_json::from_str(&content)?;
        if index < messages.len() {
            messages[index].read = true;
            let json = serde_json::to_string_pretty(&messages)?;
            fs::write(&path, json)?;
        }
        Ok(())
    })
}

/// Mark all messages as read.
///
/// Corresponds to TS: `markMessagesAsRead(name, team)`
pub fn mark_all_as_read(agent_name: &str, team_name: &str) -> Result<()> {
    let path = inbox_path(agent_name, team_name);
    if !path.exists() {
        return Ok(());
    }

    with_lock(&path, || {
        let content = fs::read_to_string(&path)?;
        let mut messages: Vec<TeammateMessage> = serde_json::from_str(&content)?;
        for msg in &mut messages {
            msg.read = true;
        }
        let json = serde_json::to_string_pretty(&messages)?;
        fs::write(&path, json)?;
        Ok(())
    })
}

/// Clear all messages from a mailbox.
///
/// No locking — overwrites with empty array.
///
/// Corresponds to TS: `clearMailbox(name, team)`
pub fn clear_mailbox(agent_name: &str, team_name: &str) -> Result<()> {
    let path = inbox_path(agent_name, team_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, "[]")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// File locking
// ---------------------------------------------------------------------------

/// Execute a closure while holding a file lock.
///
/// Uses a separate `.lock` file with exponential backoff retry.
/// Corresponds to TS: `proper-lockfile` usage.
fn with_lock<F, R>(inbox: &Path, f: F) -> Result<R>
where
    F: FnOnce() -> Result<R>,
{
    let lock = lock_path(inbox);

    // Ensure parent exists
    if let Some(parent) = lock.parent() {
        fs::create_dir_all(parent)?;
    }

    // Try to acquire lock with exponential backoff
    let mut delay_ms = MAILBOX_LOCK_MIN_TIMEOUT_MS;
    let mut acquired = false;

    for attempt in 0..MAILBOX_LOCK_RETRIES {
        // Try to create the lock file exclusively
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
        {
            Ok(mut file) => {
                // Write PID for debugging
                let _ = write!(file, "{}", std::process::id());
                acquired = true;
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                // Lock held by another process — check for stale lock
                if is_stale_lock(&lock) {
                    let _ = fs::remove_file(&lock);
                    continue;
                }
                debug!(
                    attempt,
                    delay_ms, "mailbox lock contention, backing off"
                );
                thread::sleep(Duration::from_millis(delay_ms));
                delay_ms = (delay_ms * 2).min(MAILBOX_LOCK_MAX_TIMEOUT_MS);
            }
            Err(e) => return Err(e.into()),
        }
    }

    if !acquired {
        // Force-remove stale lock as last resort
        warn!("force-removing mailbox lock after {} retries", MAILBOX_LOCK_RETRIES);
        let _ = fs::remove_file(&lock);
        // Try once more
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)
            .context("failed to acquire mailbox lock")?;
    }

    // Execute under lock
    let result = f();

    // Release lock
    let _ = fs::remove_file(&lock);

    result
}

/// Check if a lock file is stale (older than 5 seconds).
fn is_stale_lock(lock: &Path) -> bool {
    fs::metadata(lock)
        .and_then(|m| m.modified())
        .map(|modified| {
            modified
                .elapsed()
                .map(|d| d.as_secs() > 5)
                .unwrap_or(false)
        })
        .unwrap_or(true) // If we can't read metadata, treat as stale
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_team() -> (String, String, PathBuf) {
        let id = uuid::Uuid::new_v4().to_string();
        let team = format!("test-team-{}", &id[..8]);
        let agent = "test-agent";
        let dir = team_dir(&team);
        (team, agent.to_string(), dir)
    }

    fn cleanup(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("hello-world"), "hello-world");
        assert_eq!(sanitize_name("hello world"), "hello_world");
        assert_eq!(sanitize_name("test@team"), "test_team");
    }

    #[test]
    fn test_inbox_path_format() {
        let path = inbox_path("researcher", "my-team");
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("teams"));
        assert!(path_str.contains("my-team"));
        assert!(path_str.contains("inboxes"));
        assert!(path_str.contains("researcher.json"));
    }

    #[test]
    fn test_read_nonexistent_mailbox() {
        let messages = read_mailbox("nobody", "nonexistent-team-12345").unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn test_write_and_read() {
        let (team, agent, dir) = test_team();
        let msg = TeammateMessage {
            from: "sender".into(),
            text: "Hello!".into(),
            timestamp: "2026-04-01T12:00:00Z".into(),
            read: false,
            color: None,
            summary: None,
        };

        write_to_mailbox(&agent, msg, &team).unwrap();
        let messages = read_mailbox(&agent, &team).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].from, "sender");
        assert_eq!(messages[0].text, "Hello!");
        assert!(!messages[0].read);

        cleanup(&dir);
    }

    #[test]
    fn test_write_multiple_and_read() {
        let (team, agent, dir) = test_team();

        for i in 0..3 {
            let msg = TeammateMessage {
                from: format!("sender-{}", i),
                text: format!("Message {}", i),
                timestamp: "t".into(),
                read: false,
                color: None,
                summary: None,
            };
            write_to_mailbox(&agent, msg, &team).unwrap();
        }

        let messages = read_mailbox(&agent, &team).unwrap();
        assert_eq!(messages.len(), 3);

        cleanup(&dir);
    }

    #[test]
    fn test_read_unread() {
        let (team, agent, dir) = test_team();

        // Write two messages
        for i in 0..2 {
            let msg = TeammateMessage {
                from: "s".into(),
                text: format!("m{}", i),
                timestamp: "t".into(),
                read: false,
                color: None,
                summary: None,
            };
            write_to_mailbox(&agent, msg, &team).unwrap();
        }

        // Mark first as read
        mark_as_read_by_index(&agent, &team, 0).unwrap();

        let unread = read_unread_messages(&agent, &team).unwrap();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].text, "m1");

        cleanup(&dir);
    }

    #[test]
    fn test_mark_all_as_read() {
        let (team, agent, dir) = test_team();

        for _ in 0..3 {
            let msg = TeammateMessage {
                from: "s".into(),
                text: "m".into(),
                timestamp: "t".into(),
                read: false,
                color: None,
                summary: None,
            };
            write_to_mailbox(&agent, msg, &team).unwrap();
        }

        mark_all_as_read(&agent, &team).unwrap();
        let unread = read_unread_messages(&agent, &team).unwrap();
        assert!(unread.is_empty());

        cleanup(&dir);
    }

    #[test]
    fn test_clear_mailbox() {
        let (team, agent, dir) = test_team();

        let msg = TeammateMessage {
            from: "s".into(),
            text: "m".into(),
            timestamp: "t".into(),
            read: false,
            color: None,
            summary: None,
        };
        write_to_mailbox(&agent, msg, &team).unwrap();
        assert_eq!(read_mailbox(&agent, &team).unwrap().len(), 1);

        clear_mailbox(&agent, &team).unwrap();
        assert!(read_mailbox(&agent, &team).unwrap().is_empty());

        cleanup(&dir);
    }

    #[test]
    fn test_lock_path() {
        let inbox = PathBuf::from("/tmp/inbox.json");
        let lock = lock_path(&inbox);
        assert_eq!(lock, PathBuf::from("/tmp/inbox.json.lock"));
    }
}

//! Session resume picker data model.

use std::path::PathBuf;

use cc_session::storage::SessionInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionTarget {
    pub session_id: String,
    pub title: String,
    pub cwd: PathBuf,
    pub last_modified: i64,
    pub message_count: usize,
}

impl SessionTarget {
    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn from_info(info: SessionInfo) -> Self {
        Self {
            session_id: info.session_id,
            title: if info.title.trim().is_empty() {
                "(untitled session)".to_string()
            } else {
                info.title
            },
            cwd: PathBuf::from(info.cwd),
            last_modified: info.last_modified,
            message_count: info.message_count,
        }
    }
}

#[allow(dead_code)] // Phase 1: upstream parity surface
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSelection {
    Selected(SessionTarget),
    Cancelled,
}

#[allow(dead_code)] // Phase 1: upstream parity surface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionPickerAction {
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    Select,
    Cancel,
}

impl SessionPickerAction {
    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Select | Self::Cancel)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ResumePicker {
    sessions: Vec<SessionTarget>,
    selected: usize,
}

impl ResumePicker {
    pub fn new(mut sessions: Vec<SessionTarget>) -> Self {
        sessions.sort_by_key(|session| std::cmp::Reverse(session.last_modified));
        Self {
            sessions,
            selected: 0,
        }
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn from_session_info(sessions: Vec<SessionInfo>) -> Self {
        Self::new(sessions.into_iter().map(SessionTarget::from_info).collect())
    }

    pub fn selected(&self) -> Option<&SessionTarget> {
        self.sessions.get(self.selected)
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn sessions(&self) -> &[SessionTarget] {
        &self.sessions
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn apply(
        &mut self,
        action: SessionPickerAction,
        page_size: usize,
    ) -> Option<SessionSelection> {
        match action {
            SessionPickerAction::MoveUp => self.selected = self.selected.saturating_sub(1),
            SessionPickerAction::MoveDown => {
                if !self.sessions.is_empty() {
                    self.selected = (self.selected + 1).min(self.sessions.len() - 1);
                }
            }
            SessionPickerAction::PageUp => self.selected = self.selected.saturating_sub(page_size),
            SessionPickerAction::PageDown => {
                if !self.sessions.is_empty() {
                    self.selected = self
                        .selected
                        .saturating_add(page_size)
                        .min(self.sessions.len() - 1);
                }
            }
            SessionPickerAction::Select => {
                return self.selected().cloned().map(SessionSelection::Selected);
            }
            SessionPickerAction::Cancel => return Some(SessionSelection::Cancelled),
        }
        None
    }
}

#[allow(dead_code)] // Phase 1: upstream parity surface
pub fn load_resume_targets() -> anyhow::Result<Vec<SessionTarget>> {
    Ok(cc_session::storage::list_sessions()?
        .into_iter()
        .map(SessionTarget::from_info)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_newest_first() {
        let picker = ResumePicker::new(vec![
            SessionTarget {
                session_id: "old".into(),
                title: "old".into(),
                cwd: ".".into(),
                last_modified: 1,
                message_count: 0,
            },
            SessionTarget {
                session_id: "new".into(),
                title: "new".into(),
                cwd: ".".into(),
                last_modified: 2,
                message_count: 0,
            },
        ]);
        assert_eq!(picker.selected().unwrap().session_id, "new");
    }
}

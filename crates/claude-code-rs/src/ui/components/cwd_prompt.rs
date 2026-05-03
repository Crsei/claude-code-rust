//! Working-directory trust prompt model.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CwdPromptAction {
    Trust,
    ContinueOnce,
    Exit,
}

impl CwdPromptAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Trust => "Trust this workspace",
            Self::ContinueOnce => "Continue once",
            Self::Exit => "Exit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CwdSelection {
    Current(PathBuf),
    Other(PathBuf),
}

impl CwdSelection {
    pub fn path(&self) -> &Path {
        match self {
            Self::Current(path) | Self::Other(path) => path,
        }
    }

    pub fn display_name(&self) -> String {
        self.path().display().to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CwdPromptOutcome {
    Trusted(PathBuf),
    ContinueOnce(PathBuf),
    Exit,
}

pub fn resolve_cwd_prompt_outcome(
    selection: CwdSelection,
    action: CwdPromptAction,
) -> CwdPromptOutcome {
    match action {
        CwdPromptAction::Trust => CwdPromptOutcome::Trusted(selection.path().to_path_buf()),
        CwdPromptAction::ContinueOnce => {
            CwdPromptOutcome::ContinueOnce(selection.path().to_path_buf())
        }
        CwdPromptAction::Exit => CwdPromptOutcome::Exit,
    }
}

pub fn cwd_prompt_lines(selection: &CwdSelection) -> Vec<String> {
    vec![
        "Workspace trust".to_string(),
        format!("Directory: {}", selection.display_name()),
        "Choose whether this directory can run project configuration and tools.".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_trust_action_to_path() {
        let outcome = resolve_cwd_prompt_outcome(
            CwdSelection::Current(PathBuf::from("repo")),
            CwdPromptAction::Trust,
        );
        assert_eq!(outcome, CwdPromptOutcome::Trusted(PathBuf::from("repo")));
    }
}

// test infrastructure — not wired to production yet; tracked in IMPLEMENTATION_GAPS.md
//! App command model used by app-server adapters.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppCommand {
    SubmitPrompt {
        text: String,
    },
    SlashCommand {
        raw: String,
    },
    PermissionResponse {
        tool_use_id: String,
        decision: String,
    },
    Abort,
    Quit,
    ReloadConfig,
    Compact,
    SetThreadName {
        name: String,
    },
}

impl AppCommand {
    pub fn submit(text: impl Into<String>) -> Self {
        Self::SubmitPrompt { text: text.into() }
    }

    pub fn slash(raw: impl Into<String>) -> Self {
        Self::SlashCommand { raw: raw.into() }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Quit)
    }
}

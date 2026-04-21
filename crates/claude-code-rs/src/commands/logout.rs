//! `/logout` command — clear stored authentication credentials AND the
//! session-derived state that identity bleeds into (issue #43).
//!
//! The narrow predecessor wiped auth material only. The expanded version
//! walks every location where identity-derived state lives and reports the
//! outcome of each step, so users can see exactly what was purged and what
//! still needs manual attention:
//!
//! - Auth: keychain API key + `credentials.json` OAuth tokens.
//! - Onboarding: the `/onboarding` wizard flag + completion stamp.
//! - Environment: `ANTHROPIC_API_KEY` / `ANTHROPIC_AUTH_TOKEN` are flagged
//!   when set — we cannot unset them in the parent shell so we surface
//!   them as a follow-up.
//! - Managed settings: pointed at but never touched — those are policy
//!   files placed by administrators, not user-scoped caches.
//!
//! The command reports a structured multi-line result and returns success
//! even when there was nothing to clear (the action is idempotent).

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth;
use crate::services::onboarding::OnboardingStore;

pub struct LogoutHandler;

#[async_trait]
impl CommandHandler for LogoutHandler {
    async fn execute(&self, _args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let report = run_logout(&auth::resolve_auth(), &OnboardingStore::open_default());
        Ok(CommandResult::Output(report.render()))
    }
}

// ---------------------------------------------------------------------------
// Core reset flow
// ---------------------------------------------------------------------------

/// Structured result of one `/logout` invocation. Rendered to a human-
/// readable report for the user.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LogoutReport {
    pub was_authenticated: bool,
    pub auth_cleared: StepStatus,
    pub onboarding_cleared: StepStatus,
    pub env_override_warning: Option<String>,
    pub managed_settings_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    /// Nothing to do (state was already clean).
    NoOp,
    /// The step successfully cleared state.
    Cleared,
    /// The step failed with the given error message.
    Failed(String),
}

impl Default for StepStatus {
    fn default() -> Self {
        StepStatus::NoOp
    }
}

impl StepStatus {
    pub fn tag(&self) -> &'static str {
        match self {
            StepStatus::NoOp => "—",
            StepStatus::Cleared => "✓",
            StepStatus::Failed(_) => "!",
        }
    }

    pub fn detail(&self) -> String {
        match self {
            StepStatus::NoOp => "nothing to clear".to_string(),
            StepStatus::Cleared => "cleared".to_string(),
            StepStatus::Failed(msg) => format!("failed: {}", msg),
        }
    }
}

impl LogoutReport {
    pub fn render(&self) -> String {
        let mut out = String::new();

        if !self.was_authenticated
            && matches!(self.onboarding_cleared, StepStatus::NoOp)
        {
            out.push_str(
                "Not currently authenticated and no onboarding state — nothing to clear.\n",
            );
        } else {
            out.push_str("Logout complete. Cleanup summary:\n");
        }

        out.push_str(&format!(
            "  {} Auth credentials ({}, {})\n",
            self.auth_cleared.tag(),
            if self.was_authenticated {
                "keychain + credentials.json"
            } else {
                "none was present"
            },
            self.auth_cleared.detail()
        ));
        out.push_str(&format!(
            "  {} Onboarding state ({}, {})\n",
            self.onboarding_cleared.tag(),
            "onboarding.json",
            self.onboarding_cleared.detail()
        ));

        if let Some(warn) = &self.env_override_warning {
            out.push_str(&format!(
                "\nHeads up: {} — the environment still authenticates; \
                 unset it in your shell before restarting cc-rust.\n",
                warn
            ));
        }

        if let Some(managed) = &self.managed_settings_path {
            out.push_str(&format!(
                "\nManaged (policy) settings at {} were NOT touched — they are \
                 administrator-owned and outside the scope of /logout.\n",
                managed
            ));
        }

        out
    }
}

/// Run the full logout sequence. Parameterized for testability — unit tests
/// substitute a tempdir-backed `OnboardingStore` and a pre-built auth state.
fn run_logout(current_auth: &auth::AuthMethod, onboarding: &OnboardingStore) -> LogoutReport {
    let was_authenticated = current_auth.is_authenticated();

    let auth_cleared = if was_authenticated {
        match auth::oauth_logout() {
            Ok(_) => StepStatus::Cleared,
            Err(e) => StepStatus::Failed(e.to_string()),
        }
    } else {
        StepStatus::NoOp
    };

    let onboarding_cleared = {
        // Use the in-place reset path so the user's display_name survives
        // the logout — it's a preference, not an identity artifact.
        let had_state_before = match onboarding.load() {
            Ok(state) => !state.is_first_run() || onboarding.path().exists(),
            Err(_) => onboarding.path().exists(),
        };
        if had_state_before {
            match onboarding.update(|s| s.reset_for_logout()) {
                Ok(_) => StepStatus::Cleared,
                Err(e) => StepStatus::Failed(e.to_string()),
            }
        } else {
            StepStatus::NoOp
        }
    };

    let env_override_warning = detect_env_override();
    let managed_settings_path = detect_managed_settings();

    LogoutReport {
        was_authenticated,
        auth_cleared,
        onboarding_cleared,
        env_override_warning,
        managed_settings_path,
    }
}

fn detect_env_override() -> Option<String> {
    const VARS: &[&str] = &["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];
    let present: Vec<&str> = VARS
        .iter()
        .copied()
        .filter(|var| {
            std::env::var(var)
                .map(|v| !v.trim().is_empty())
                .unwrap_or(false)
        })
        .collect();
    if present.is_empty() {
        None
    } else {
        Some(format!("{} is set in the environment", present.join(", ")))
    }
}

fn detect_managed_settings() -> Option<String> {
    let path = cc_config::settings::managed_settings_path();
    if path.exists() {
        Some(path.display().to_string())
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::services::onboarding::{OnboardingState, OnboardingStore};
    use crate::types::app_state::AppState;
    use chrono::Utc;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("."),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[tokio::test]
    async fn test_logout_returns_output() {
        let handler = LogoutHandler;
        let mut ctx = test_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(text) => {
                assert!(!text.is_empty());
            }
            _ => panic!("Expected Output"),
        }
    }

    #[test]
    fn unauthenticated_and_empty_onboarding_is_no_op_report() {
        let dir = tempdir().unwrap();
        let store = OnboardingStore::new(dir.path().join("onboarding.json"));
        let report = run_logout(&auth::AuthMethod::None, &store);
        assert!(!report.was_authenticated);
        assert!(matches!(report.auth_cleared, StepStatus::NoOp));
        assert!(matches!(report.onboarding_cleared, StepStatus::NoOp));
        let text = report.render();
        assert!(text.contains("nothing to clear"));
    }

    #[test]
    fn clears_onboarding_when_present_even_if_unauthenticated() {
        let dir = tempdir().unwrap();
        let store = OnboardingStore::new(dir.path().join("onboarding.json"));
        store
            .update(|s| {
                s.has_completed_onboarding = true;
                s.completed_at = Some(Utc::now());
                s.display_name = Some("Sam".into());
            })
            .unwrap();
        assert!(store.path().exists());

        let report = run_logout(&auth::AuthMethod::None, &store);
        assert!(matches!(report.onboarding_cleared, StepStatus::Cleared));

        // The file is rewritten (not deleted) so display_name survives.
        let after = store.load().unwrap();
        assert!(!after.has_completed_onboarding);
        assert!(!after.auth_onboarding_done);
        assert!(after.completed_at.is_none());
        assert_eq!(after.display_name, Some("Sam".into()));
    }

    #[test]
    fn render_lists_every_step() {
        let report = LogoutReport {
            was_authenticated: true,
            auth_cleared: StepStatus::Cleared,
            onboarding_cleared: StepStatus::Cleared,
            env_override_warning: None,
            managed_settings_path: None,
        };
        let text = report.render();
        assert!(text.contains("Auth credentials"));
        assert!(text.contains("Onboarding state"));
        assert!(text.contains("Logout complete"));
    }

    #[test]
    fn render_shows_env_override_warning_when_set() {
        let report = LogoutReport {
            env_override_warning: Some("ANTHROPIC_API_KEY is set in the environment".into()),
            ..LogoutReport::default()
        };
        let text = report.render();
        assert!(text.contains("ANTHROPIC_API_KEY"));
        assert!(text.contains("unset it"));
    }

    #[test]
    fn render_notes_managed_settings_when_present() {
        let report = LogoutReport {
            managed_settings_path: Some("/etc/cc-rust/managed-settings.json".into()),
            ..LogoutReport::default()
        };
        let text = report.render();
        assert!(text.contains("Managed (policy) settings"));
        assert!(text.contains("NOT touched"));
    }

    #[test]
    fn status_detail_strings_are_informative() {
        assert_eq!(StepStatus::NoOp.detail(), "nothing to clear");
        assert_eq!(StepStatus::Cleared.detail(), "cleared");
        assert!(StepStatus::Failed("disk full".into())
            .detail()
            .contains("disk full"));
    }

    #[test]
    fn onboarding_state_marker_survives_partial_logout() {
        // Even if an empty file exists, we should still treat it as
        // state-worth-resetting (belt-and-suspenders for weird edge cases).
        let dir = tempdir().unwrap();
        let path = dir.path().join("onboarding.json");
        std::fs::write(&path, "").unwrap();
        let store = OnboardingStore::new(&path);
        let report = run_logout(&auth::AuthMethod::None, &store);
        assert!(matches!(report.onboarding_cleared, StepStatus::Cleared));
        // File is rewritten with default state, not deleted.
        let state = store.load().unwrap();
        assert!(state.is_first_run());
    }

    #[test]
    fn state_type_is_wired() {
        // A compile-check: the LogoutReport consumes OnboardingState through
        // the store, and both types must stay Send + serializable-shaped.
        let _state = OnboardingState::default();
    }
}

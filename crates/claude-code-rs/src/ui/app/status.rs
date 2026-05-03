use crate::config::settings::StatusLineSettings;
use crate::ui::status_line::{
    build_payload_from_snapshot, payload, StatusLinePayload, StatusLineRunner, StatusLineSnapshot,
};

use super::App;
/// Subset of engine usage-tracking relevant to the status-line payload.
/// Populated by [`App::update_session_usage`].
#[derive(Debug, Clone, Default)]
pub(super) struct SessionUsageSnapshot {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub api_calls: u64,
}

impl App {
    /// Update the accumulated session usage (tokens + api calls) used by
    /// the scriptable status-line payload.
    pub fn update_session_usage(
        &mut self,
        input_tokens: u64,
        output_tokens: u64,
        cache_read_tokens: u64,
        cache_creation_tokens: u64,
        api_calls: u64,
    ) {
        self.session_usage = SessionUsageSnapshot {
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            api_calls,
        };
        // No dirty flip; `update_session_cost` already ran and marked it.
    }

    /// Replace the resolved status-line settings (e.g. after `/statusline`
    /// edits the config).
    pub fn set_status_line_settings(&mut self, settings: StatusLineSettings) {
        self.status_line_settings = settings;
        // Drop any stale output so we fall back immediately if the user
        // disabled or cleared the command.
        if !self.status_line_settings.is_command_mode() {
            self.status_line_runner.reset();
        }
        self.dirty = true;
    }

    /// Shared handle to the status-line runner. `/statusline` calls this
    /// to inspect / reset the runner without owning the App.
    pub fn status_line_runner(&self) -> StatusLineRunner {
        self.status_line_runner.clone()
    }

    /// Adopt a pre-existing runner (e.g. the one stored on [`AppState`])
    /// so every UI surface and the `/statusline` command observe the same
    /// subprocess state.
    pub fn set_status_line_runner(&mut self, runner: StatusLineRunner) {
        self.status_line_runner = runner;
    }

    /// Build the current status-line payload from app state.
    pub(super) fn build_status_payload(&self) -> StatusLinePayload {
        let mut payload = build_payload_from_snapshot(StatusLineSnapshot {
            session_id: (!self.session_id.is_empty()).then(|| self.session_id.clone()),
            model_id: &self.model_name,
            backend: (!self.backend_name.is_empty()).then_some(self.backend_name.as_str()),
            cwd: std::path::Path::new(&self.cwd),
            input_tokens: self.session_usage.input_tokens,
            output_tokens: self.session_usage.output_tokens,
            cache_read_tokens: self.session_usage.cache_read_tokens,
            cache_creation_tokens: self.session_usage.cache_creation_tokens,
            total_cost_usd: self.session_cost_usd,
            api_calls: self.session_usage.api_calls,
            session_duration_secs: None,
            resolved_output_style_name: crate::ui::status_line_resolver::resolve_output_style_name(
                self.output_style.as_deref(),
                std::path::Path::new(&self.cwd),
            ),
            editor_mode: self.vim.enabled.then_some("vim"),
            worktree: crate::ui::status_line_resolver::current_worktree_status(),
            streaming: self.is_streaming,
            message_count: self.messages.len(),
        });
        if self.vim.enabled {
            payload.vim = Some(payload::VimStatus {
                mode: self.vim.mode.indicator().to_string(),
            });
        }
        payload
    }

    /// Kick the runner. Throttling / cancellation lives inside the runner.
    pub(super) fn trigger_status_refresh(&self) {
        if !self.status_line_settings.is_command_mode() {
            return;
        }
        let payload = self.build_status_payload();
        let _ = self
            .status_line_runner
            .refresh(&self.status_line_settings, &payload);
    }
}

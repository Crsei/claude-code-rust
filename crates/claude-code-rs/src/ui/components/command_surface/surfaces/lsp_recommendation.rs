use crossterm::event::{KeyCode, KeyEvent};

use crate::ipc::subsystem_types::LspRecommendationPayload;
use crate::ui::command_surface::CommandSurfaceOutcome;
use crate::ui::lsp_recommendation::lsp_recommendation_menu::{
    LspRecommendationDecision, LspRecommendationPromptState,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRecommendationSurface {
    pub(crate) request_id: String,
    pub(crate) state: LspRecommendationPromptState,
}

impl LspRecommendationSurface {
    pub(crate) fn new(payload: LspRecommendationPayload) -> Self {
        Self {
            request_id: payload.request_id,
            state: LspRecommendationPromptState::new(
                payload.plugin_name,
                payload.plugin_description,
                payload.file_extension,
            ),
        }
    }
    pub(crate) fn render(&self) -> String {
        self.state.render()
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_prev();
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.state.move_next();
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self.response(self.state.selected_decision()),
            _ => CommandSurfaceOutcome::None,
        }
    }

    pub(crate) fn cancel(&self) -> CommandSurfaceOutcome {
        self.response(LspRecommendationDecision::No)
    }

    pub(crate) fn response(&self, decision: LspRecommendationDecision) -> CommandSurfaceOutcome {
        let install_prompt = (decision == LspRecommendationDecision::Yes)
            .then(|| format!("/plugin install {} ", self.state.plugin_name));
        CommandSurfaceOutcome::LspRecommendationResponse {
            request_id: self.request_id.clone(),
            plugin_name: self.state.plugin_name.clone(),
            decision: decision.value().to_string(),
            install_prompt,
        }
    }
}

use crossterm::event::KeyEvent;

use crate::types::app_state::AppState;
use crate::ui::command_surface::CommandSurfaceOutcome;
use crate::ui::form_navigation::{FormOption, FormTab, TabbedFormEvent, TabbedFormState};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSurface {
    pub(crate) state: TabbedFormState,
}

impl ConfigSurface {
    pub(crate) fn new(state: &AppState) -> Self {
        let model = if state.main_loop_model.is_empty() {
            "default".to_string()
        } else {
            state.main_loop_model.clone()
        };
        let backend = if state.main_loop_backend.is_empty() {
            "default".to_string()
        } else {
            state.main_loop_backend.clone()
        };
        let editor = state
            .settings
            .editor_mode
            .clone()
            .unwrap_or_else(|| "normal".to_string());

        Self {
            state: TabbedFormState::new(
                "Config",
                vec![
                    FormTab::new(
                        "status",
                        "Status",
                        vec![
                            FormOption::new("show", "Show effective config")
                                .with_description(format!("model={model}; backend={backend}")),
                            FormOption::new("sources", "Show setting sources")
                                .with_description("which layer set each key"),
                        ],
                    ),
                    FormTab::new(
                        "config",
                        "Config",
                        vec![
                            FormOption::new("raw", "Show raw layers")
                                .with_description("managed/user/project/local settings"),
                            FormOption::new("schema", "Show schema")
                                .with_description("JSON schema for settings.json"),
                            FormOption::new("set-model", "Set model")
                                .with_description("fill prompt with /config set model"),
                        ],
                    ),
                    FormTab::new(
                        "editor",
                        "Editor",
                        vec![
                            FormOption::new("set-vim", "Enable vim mode")
                                .with_description(format!("current editorMode={editor}")),
                            FormOption::new("set-normal", "Use normal editor mode")
                                .with_description(format!("current editorMode={editor}")),
                        ],
                    ),
                ],
            ),
        }
    }

    pub(crate) fn render(&self) -> String {
        self.state.render_lines().join("\n")
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match self.state.handle_key(key) {
            TabbedFormEvent::Selected { option_id, .. } => match option_id.as_str() {
                "show" => CommandSurfaceOutcome::Submit("/config show".to_string()),
                "sources" => CommandSurfaceOutcome::Submit("/config sources".to_string()),
                "raw" => CommandSurfaceOutcome::Submit("/config show --raw".to_string()),
                "schema" => CommandSurfaceOutcome::Submit("/config schema".to_string()),
                "set-model" => CommandSurfaceOutcome::FillPrompt("/config set model ".to_string()),
                "set-vim" => {
                    CommandSurfaceOutcome::Submit("/config set editorMode vim".to_string())
                }
                "set-normal" => {
                    CommandSurfaceOutcome::Submit("/config set editorMode normal".to_string())
                }
                _ => CommandSurfaceOutcome::None,
            },
            _ => CommandSurfaceOutcome::None,
        }
    }
}

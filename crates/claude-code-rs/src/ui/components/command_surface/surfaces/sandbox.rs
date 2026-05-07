use crossterm::event::KeyEvent;

use crate::types::app_state::AppState;
use crate::ui::command_surface::CommandSurfaceOutcome;
use crate::ui::form_navigation::{FormOption, FormTab, TabbedFormEvent, TabbedFormState};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxSurface {
    pub(crate) state: TabbedFormState,
}

impl SandboxSurface {
    pub(crate) fn new(state: &AppState) -> Self {
        let enabled = state
            .settings
            .sandbox
            .enabled
            .map(|value| if value { "enabled" } else { "disabled" })
            .unwrap_or("default");
        let mode = state
            .settings
            .sandbox
            .mode
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let network = state
            .settings
            .sandbox
            .network
            .disabled
            .map(|value| if value { "disabled" } else { "enabled" })
            .unwrap_or("default");

        Self {
            state: TabbedFormState::new(
                "Sandbox",
                vec![
                    FormTab::new(
                        "status",
                        "Status",
                        vec![
                            FormOption::new("status", "Show sandbox status")
                                .with_description(format!("sandbox={enabled}; mode={mode}")),
                        ],
                    ),
                    FormTab::new(
                        "mode",
                        "Mode",
                        vec![
                            FormOption::new("on", "Enable sandbox")
                                .with_description("session override: enabled=true"),
                            FormOption::new("off", "Disable sandbox")
                                .with_description("session override: enabled=false"),
                            FormOption::new("read-only", "Read-only mode")
                                .with_description(format!("current mode={mode}")),
                            FormOption::new("workspace", "Workspace mode")
                                .with_description(format!("current mode={mode}")),
                            FormOption::new("full", "Full mode")
                                .with_description(format!("current mode={mode}")),
                        ],
                    ),
                    FormTab::new(
                        "network",
                        "Network",
                        vec![
                            FormOption::new("network-on", "Enable network")
                                .with_description(format!("current network={network}")),
                            FormOption::new("network-off", "Disable network")
                                .with_description(format!("current network={network}")),
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
                "status" => CommandSurfaceOutcome::Submit("/sandbox status".to_string()),
                "on" => CommandSurfaceOutcome::Submit("/sandbox on".to_string()),
                "off" => CommandSurfaceOutcome::Submit("/sandbox off".to_string()),
                "read-only" => CommandSurfaceOutcome::Submit("/sandbox mode read-only".to_string()),
                "workspace" => CommandSurfaceOutcome::Submit("/sandbox mode workspace".to_string()),
                "full" => CommandSurfaceOutcome::Submit("/sandbox mode full".to_string()),
                "network-on" => CommandSurfaceOutcome::Submit("/sandbox network on".to_string()),
                "network-off" => CommandSurfaceOutcome::Submit("/sandbox network off".to_string()),
                _ => CommandSurfaceOutcome::None,
            },
            _ => CommandSurfaceOutcome::None,
        }
    }
}

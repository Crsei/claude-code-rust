use crossterm::event::KeyEvent;

use crate::engine::effort::effort_to_budget_tokens;
use crate::model_registry;
use crate::types::app_state::AppState;
use crate::ui::command_surface::CommandSurfaceOutcome;
use crate::ui::form_navigation::{FormOption, FormTab, TabbedFormEvent, TabbedFormState};
use crate::ui::keyboard_shortcut::{render_shortcut_hints, ShortcutHint};
use crate::ui::selection_surface::{SelectionItem, SelectionSurface, SelectionSurfaceEvent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSurface {
    pub(crate) state: TabbedFormState,
    model_picker: SelectionSurface,
    theme_picker: SelectionSurface,
    effort_picker: SelectionSurface,
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
                        "model",
                        "Model",
                        vec![FormOption::new("picker", "Model picker").disabled()],
                    ),
                    FormTab::new(
                        "theme",
                        "Theme",
                        vec![FormOption::new("picker", "Theme picker").disabled()],
                    ),
                    FormTab::new(
                        "effort",
                        "Effort",
                        vec![FormOption::new("picker", "Effort picker").disabled()],
                    ),
                    FormTab::new(
                        "config",
                        "Config",
                        vec![
                            FormOption::new("raw", "Show raw layers")
                                .with_description("managed/user/project/local settings"),
                            FormOption::new("schema", "Show schema")
                                .with_description("JSON schema for settings.json"),
                            FormOption::new("set-model", "Set custom model")
                                .with_description("fill prompt with /config set model"),
                            FormOption::new("set-theme", "Set custom theme")
                                .with_description("fill prompt with /config set theme"),
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
            model_picker: build_model_picker(state),
            theme_picker: build_theme_picker(state),
            effort_picker: build_effort_picker(state),
        }
    }

    pub(crate) fn render(&self) -> String {
        match self.active_tab_id() {
            Some("model") => {
                let effort = self.current_effort_label();
                self.render_picker(
                    &self.model_picker,
                    &[format!(
                        "Current effort: {}",
                        format_effort(effort.as_deref())
                    )],
                )
            }
            Some("theme") => self.render_picker(&self.theme_picker, &[]),
            Some("effort") => self.render_picker(&self.effort_picker, &[]),
            _ => self.state.render_lines().join("\n"),
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        if is_tab_navigation_key(&key) {
            let _ = self.state.handle_key(key);
            return CommandSurfaceOutcome::None;
        }

        match self.active_tab_id() {
            Some("model") => {
                return handle_picker_key(&mut self.model_picker, key, |id| {
                    CommandSurfaceOutcome::Submit(format!("/config set model {id}"))
                });
            }
            Some("theme") => {
                return handle_picker_key(&mut self.theme_picker, key, |id| {
                    CommandSurfaceOutcome::Submit(format!("/config set theme {id}"))
                });
            }
            Some("effort") => {
                return handle_picker_key(&mut self.effort_picker, key, |id| {
                    CommandSurfaceOutcome::Submit(format!("/config set effortLevel {id}"))
                });
            }
            _ => {}
        }

        match self.state.handle_key(key) {
            TabbedFormEvent::Selected { option_id, .. } => match option_id.as_str() {
                "show" => CommandSurfaceOutcome::Submit("/config show".to_string()),
                "sources" => CommandSurfaceOutcome::Submit("/config sources".to_string()),
                "raw" => CommandSurfaceOutcome::Submit("/config show --raw".to_string()),
                "schema" => CommandSurfaceOutcome::Submit("/config schema".to_string()),
                "set-model" => CommandSurfaceOutcome::FillPrompt("/config set model ".to_string()),
                "set-theme" => CommandSurfaceOutcome::FillPrompt("/config set theme ".to_string()),
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

    fn active_tab_id(&self) -> Option<&str> {
        self.state.active_tab().map(|tab| tab.id.as_str())
    }

    fn current_effort_label(&self) -> Option<String> {
        self.effort_picker
            .items
            .iter()
            .find(|item| item.description.contains("current"))
            .map(|item| item.id.clone())
    }

    fn render_picker(&self, picker: &SelectionSurface, context_lines: &[String]) -> String {
        let mut lines = vec![self.state.title.clone(), render_tab_line(&self.state)];
        lines.extend(context_lines.iter().cloned());
        lines.extend(picker.render_lines(10));
        lines.push(render_shortcut_hints(&[
            ShortcutHint::new("Left/Right", "switch tabs"),
            ShortcutHint::new("Type", "filter"),
            ShortcutHint::new("Up/Down", "navigate"),
            ShortcutHint::new("Enter", "select"),
            ShortcutHint::new("Esc", "close"),
        ]));
        lines.join("\n")
    }
}

fn handle_picker_key(
    picker: &mut SelectionSurface,
    key: KeyEvent,
    submit: impl FnOnce(String) -> CommandSurfaceOutcome,
) -> CommandSurfaceOutcome {
    match picker.handle_key(key) {
        SelectionSurfaceEvent::Selected(id) => submit(id),
        SelectionSurfaceEvent::Closed => CommandSurfaceOutcome::Close,
        SelectionSurfaceEvent::None => CommandSurfaceOutcome::None,
    }
}

fn is_tab_navigation_key(key: &KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyEventKind};

    if key.kind != KeyEventKind::Press {
        return false;
    }

    matches!(
        key.code,
        KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab
    ) || matches!(key.code, KeyCode::Char(ch) if ch.is_ascii_digit())
}

fn render_tab_line(state: &TabbedFormState) -> String {
    crate::ui::tabs::render_tabs(
        &state
            .tabs
            .iter()
            .map(|tab| tab.label.as_str())
            .collect::<Vec<_>>(),
        state.active_tab,
    )
}

fn build_model_picker(state: &AppState) -> SelectionSurface {
    let current = if state.main_loop_model.is_empty() {
        AppState::default().main_loop_model
    } else {
        state.main_loop_model.clone()
    };
    let effort = state
        .effort_value
        .as_deref()
        .or(state.settings.effort_level.as_deref());
    let mut items = Vec::new();

    if state.settings.available_models.is_empty() {
        for entry in model_registry::MODEL_ALIASES {
            push_model_item(
                &mut items,
                entry.target,
                &current,
                Some(entry.alias),
                "built-in alias",
                effort,
            );
        }
    } else {
        for configured in &state.settings.available_models {
            let resolved = model_registry::resolve_model_alias(configured);
            push_model_item(
                &mut items,
                &resolved,
                &current,
                model_registry::alias_for_model(&resolved),
                "configured",
                effort,
            );
        }
    }

    if !items.iter().any(|item| item.id == current) {
        push_model_item(
            &mut items,
            &current,
            &current,
            model_registry::alias_for_model(&current),
            "current custom",
            effort,
        );
    }

    let selected = items
        .iter()
        .position(|item| item.id == current)
        .unwrap_or(0);
    let mut picker = SelectionSurface::new("Model", items);
    picker.selected = selected;
    picker
}

fn push_model_item(
    items: &mut Vec<SelectionItem>,
    model: &str,
    current: &str,
    alias: Option<&str>,
    source: &str,
    effort: Option<&str>,
) {
    if items.iter().any(|item| item.id == model) {
        return;
    }

    let label = alias
        .map(|alias| format!("{alias} ({model})"))
        .unwrap_or_else(|| model.to_string());
    let mut description = vec![source.to_string()];
    if model == current {
        description.push("current".to_string());
    }
    if let Some(effort) = effort {
        description.push(format!("effort={effort}"));
    }

    items.push(SelectionItem {
        id: model.to_string(),
        label,
        description: description.join("; "),
        enabled: true,
        disabled_reason: None,
        preview_lines: Vec::new(),
        actions: Vec::new(),
        search_terms: Vec::new(),
    });
}

fn build_theme_picker(state: &AppState) -> SelectionSurface {
    let current = state.settings.theme.as_deref().unwrap_or("dark");
    let mut items = vec![
        theme_item("auto", "Auto", "match terminal when supported", current),
        theme_item("dark", "Dark mode", "default ratatui palette", current),
        theme_item("light", "Light mode", "light terminal palette", current),
        theme_item(
            "solarized",
            "Solarized",
            "low-contrast terminal palette",
            current,
        ),
        theme_item(
            "monokai",
            "Monokai",
            "high-contrast editor palette",
            current,
        ),
        theme_item("nord", "Nord", "cool low-saturation palette", current),
    ];

    if !items.iter().any(|item| item.id == current) {
        items.push(theme_item(
            current,
            format!("Custom ({current})"),
            "current custom theme",
            current,
        ));
    }

    let selected = items
        .iter()
        .position(|item| item.id == current)
        .unwrap_or(1);
    let mut picker = SelectionSurface::new("Theme", items);
    picker.selected = selected;
    picker
}

fn theme_item(
    id: impl Into<String>,
    label: impl Into<String>,
    description: impl Into<String>,
    current: &str,
) -> SelectionItem {
    let id = id.into();
    let mut description = description.into();
    if id == current {
        if description.is_empty() {
            description = "current".to_string();
        } else {
            description.push_str("; current");
        }
    }
    SelectionItem {
        id,
        label: label.into(),
        description,
        enabled: true,
        disabled_reason: None,
        preview_lines: Vec::new(),
        actions: Vec::new(),
        search_terms: Vec::new(),
    }
}

fn build_effort_picker(state: &AppState) -> SelectionSurface {
    let current = state
        .effort_value
        .as_deref()
        .or(state.settings.effort_level.as_deref());
    let mut items = vec![
        effort_item("auto", "Auto", "model default", current),
        effort_item("low", "Low", "shorter thinking budget", current),
        effort_item("medium", "Medium", "balanced thinking budget", current),
        effort_item("high", "High", "deeper thinking budget", current),
        effort_item("max", "Max", "largest fixed thinking budget", current),
    ];

    if let Some(current) = current {
        if !items.iter().any(|item| item.id == current) {
            items.push(effort_item(
                current,
                format!("Custom ({current})"),
                "current",
                Some(current),
            ));
        }
    }

    let selected = current
        .and_then(|value| items.iter().position(|item| item.id == value))
        .unwrap_or(0);
    let mut picker = SelectionSurface::new("Effort", items);
    picker.selected = selected;
    picker
}

fn effort_item(
    id: impl Into<String>,
    label: impl Into<String>,
    description: impl Into<String>,
    current: Option<&str>,
) -> SelectionItem {
    let id = id.into();
    let mut details = vec![description.into()];
    if let Some(tokens) = effort_to_budget_tokens(&id) {
        details.push(format!("{tokens} tokens"));
    }
    if current == Some(id.as_str()) {
        details.push("current".to_string());
    }

    SelectionItem {
        id,
        label: label.into(),
        description: details.join("; "),
        enabled: true,
        disabled_reason: None,
        preview_lines: Vec::new(),
        actions: Vec::new(),
        search_terms: Vec::new(),
    }
}

fn format_effort(value: Option<&str>) -> String {
    value
        .map(|value| match effort_to_budget_tokens(value) {
            Some(tokens) => format!("{value} ({tokens} tokens)"),
            None => value.to_string(),
        })
        .unwrap_or_else(|| "not set".to_string())
}

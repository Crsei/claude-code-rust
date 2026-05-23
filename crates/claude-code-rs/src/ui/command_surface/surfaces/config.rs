use crossterm::event::KeyEvent;

use crate::ui::better_view_panel::BetterViewPanel;
use crate::ui::command_surface::CommandSurfaceOutcome;
use crate::ui::form_navigation::{FormOption, FormTab, TabbedFormEvent, TabbedFormState};
use crate::ui::selection_surface::{SelectionItem, SelectionSurface, SelectionSurfaceEvent};
use cc_engine::effort::effort_to_budget_tokens;
use cc_engine::types::app_state::AppState;
use cc_models::aliases as model_registry;

fn neutral_model_alias(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    if trimmed.eq_ignore_ascii_case("SOTA") {
        Some("SOTA")
    } else if trimmed.eq_ignore_ascii_case("MOTA") {
        Some("MOTA")
    } else if trimmed.eq_ignore_ascii_case("FOTA") {
        Some("FOTA")
    } else {
        None
    }
}

fn anthropic_provider_selected(state: &AppState) -> bool {
    state
        .settings
        .api_provider
        .as_deref()
        .and_then(cc_config::settings::normalize_api_provider)
        == Some(cc_config::settings::API_PROVIDER_ANTHROPIC)
        || std::env::var_os("ANTHROPIC_BASE_URL").is_some()
        || std::env::var_os("ANTHROPIC_DEFAULT_SOTA_MODEL").is_some()
        || std::env::var_os("ANTHROPIC_DEFAULT_MOTA_MODEL").is_some()
        || std::env::var_os("ANTHROPIC_DEFAULT_FOTA_MODEL").is_some()
        || std::env::var_os("ANTHROPIC_DEFAULT_OPUS_MODEL").is_some()
        || std::env::var_os("ANTHROPIC_DEFAULT_SONNET_MODEL").is_some()
        || std::env::var_os("ANTHROPIC_DEFAULT_HAIKU_MODEL").is_some()
}

fn anthropic_alias_model(alias: &str) -> Option<String> {
    let (env_name, legacy_env_name) = match neutral_model_alias(alias)? {
        "SOTA" => (
            "ANTHROPIC_DEFAULT_SOTA_MODEL",
            "ANTHROPIC_DEFAULT_OPUS_MODEL",
        ),
        "MOTA" => (
            "ANTHROPIC_DEFAULT_MOTA_MODEL",
            "ANTHROPIC_DEFAULT_SONNET_MODEL",
        ),
        "FOTA" => (
            "ANTHROPIC_DEFAULT_FOTA_MODEL",
            "ANTHROPIC_DEFAULT_HAIKU_MODEL",
        ),
        _ => return None,
    };
    std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var(legacy_env_name)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}

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
        let output_style = state
            .settings
            .output_style
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let language = state
            .settings
            .language
            .clone()
            .unwrap_or_else(|| "not set".to_string());
        let editor = state
            .settings
            .editor_mode
            .clone()
            .unwrap_or_else(|| "normal".to_string());
        let fast_mode = if state.fast_mode {
            "true".to_string()
        } else {
            bool_setting_label(state.settings.fast_mode)
        };
        let thinking = state
            .thinking_enabled
            .map(|enabled| enabled.to_string())
            .unwrap_or_else(|| "auto".to_string());

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
                        "usage",
                        "Usage",
                        vec![
                            FormOption::new("cost", "Show session usage")
                                .with_description("token totals and estimated cost"),
                            FormOption::new("extra-usage", "Show extended usage")
                                .with_description("per-message token and cost breakdown"),
                        ],
                    ),
                    FormTab::new(
                        "output",
                        "Output",
                        vec![
                            FormOption::new("style-default", "Use default output style")
                                .with_description(format!("current outputStyle={output_style}")),
                            FormOption::new("style-explanatory", "Use explanatory output style")
                                .with_description(format!("current outputStyle={output_style}")),
                            FormOption::new("style-learning", "Use learning output style")
                                .with_description(format!("current outputStyle={output_style}")),
                            FormOption::new("set-output-style", "Set custom output style")
                                .with_description("fill prompt with /config set outputStyle"),
                            FormOption::new("editor-vim", "Enable vim editor mode")
                                .with_description(format!("current editorMode={editor}")),
                            FormOption::new("editor-normal", "Use normal editor mode")
                                .with_description(format!("current editorMode={editor}")),
                        ],
                    ),
                    FormTab::new(
                        "language",
                        "Language",
                        vec![
                            FormOption::new("set-language", "Set response language")
                                .with_description(format!("current language={language}")),
                            FormOption::new("language-en", "Use English")
                                .with_description(format!("current language={language}")),
                            FormOption::new("language-zh", "Use Chinese")
                                .with_description(format!("current language={language}")),
                        ],
                    ),
                    FormTab::new(
                        "thinking",
                        "Thinking",
                        vec![FormOption::new("picker", "Effort picker")
                            .with_description(format!(
                                "current effort={}; thinking={thinking}; fastMode={fast_mode}; selection applies immediately",
                                state
                                    .effort_value
                                    .as_deref()
                                    .or(state.settings.effort_level.as_deref())
                                    .unwrap_or("auto")
                            ))
                            .disabled()],
                    ),
                    FormTab::new(
                        "safety",
                        "Safety",
                        vec![FormOption::new("sources", "Review setting sources")
                            .with_description("managed/user/project/local provenance")],
                    ),
                    FormTab::new(
                        "config",
                        "Config",
                        vec![
                            FormOption::new("set-model", "Set custom model")
                                .with_description("fill prompt with /config set model"),
                            FormOption::new("set-theme", "Set custom theme")
                                .with_description("fill prompt with /config set theme"),
                        ],
                    ),
                ],
            ),
            model_picker: build_model_picker(state),
            theme_picker: build_theme_picker(state),
            effort_picker: build_effort_picker(state),
        }
    }

    pub(crate) fn new_thinking_picker(state: &AppState) -> Self {
        let mut surface = Self::new(state);
        if let Some(index) = surface
            .state
            .tabs
            .iter()
            .position(|tab| tab.id == "thinking")
        {
            surface.state.active_tab = index;
        }
        surface
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
            Some("thinking") => self.render_picker(&self.effort_picker, &[]),
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
            Some("thinking") => {
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
                "cost" => CommandSurfaceOutcome::Submit("/cost".to_string()),
                "extra-usage" => CommandSurfaceOutcome::Submit("/extra-usage".to_string()),
                "style-default" => {
                    CommandSurfaceOutcome::Submit("/config set outputStyle default".to_string())
                }
                "style-explanatory" => {
                    CommandSurfaceOutcome::Submit("/config set outputStyle explanatory".to_string())
                }
                "style-learning" => {
                    CommandSurfaceOutcome::Submit("/config set outputStyle learning".to_string())
                }
                "set-output-style" => {
                    CommandSurfaceOutcome::FillPrompt("/config set outputStyle ".to_string())
                }
                "set-language" => {
                    CommandSurfaceOutcome::FillPrompt("/config set language ".to_string())
                }
                "language-en" => {
                    CommandSurfaceOutcome::Submit("/config set language English".to_string())
                }
                "language-zh" => {
                    CommandSurfaceOutcome::Submit("/config set language Chinese".to_string())
                }
                "voice-on" => {
                    CommandSurfaceOutcome::Submit("/config set voiceEnabled true".to_string())
                }
                "voice-off" => {
                    CommandSurfaceOutcome::Submit("/config set voiceEnabled false".to_string())
                }
                "editor-vim" => {
                    CommandSurfaceOutcome::Submit("/config set editorMode vim".to_string())
                }
                "editor-normal" => {
                    CommandSurfaceOutcome::Submit("/config set editorMode normal".to_string())
                }
                "progress-on" => CommandSurfaceOutcome::Submit(
                    "/config set terminalProgressBarEnabled true".to_string(),
                ),
                "progress-off" => CommandSurfaceOutcome::Submit(
                    "/config set terminalProgressBarEnabled false".to_string(),
                ),
                "raw" => CommandSurfaceOutcome::Submit("/config show --raw".to_string()),
                "schema" => CommandSurfaceOutcome::Submit("/config schema".to_string()),
                "set-model" => CommandSurfaceOutcome::FillPrompt("/config set model ".to_string()),
                "set-theme" => CommandSurfaceOutcome::FillPrompt("/config set theme ".to_string()),
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
        let mut lines = context_lines.to_vec();
        lines.extend(picker.render_lines(10));
        BetterViewPanel::new(&self.state.title)
            .sections(
                self.state
                    .tabs
                    .iter()
                    .map(|tab| tab.label.clone())
                    .collect::<Vec<_>>(),
                self.state.active_tab,
            )
            .detail_title(picker.title.clone())
            .detail_lines(lines)
            .footer(
                "Type filter | Up/Down navigate | Enter select | Left/Right section | Esc close",
            )
            .render()
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

pub(super) fn build_model_picker(state: &AppState) -> SelectionSurface {
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
            let model = if anthropic_provider_selected(state) {
                anthropic_alias_model(entry.alias).unwrap_or_else(|| entry.target.to_string())
            } else {
                entry.target.to_string()
            };
            push_model_item(
                &mut items,
                entry.alias,
                &model,
                &current,
                Some(entry.alias),
                "built-in alias",
                effort,
            );
        }
    } else {
        for configured in &state.settings.available_models {
            let alias = neutral_model_alias(configured);
            let resolved = if anthropic_provider_selected(state) {
                alias
                    .and_then(anthropic_alias_model)
                    .unwrap_or_else(|| model_registry::resolve_model_alias(configured))
            } else {
                model_registry::resolve_model_alias(configured)
            };
            push_model_item(
                &mut items,
                alias.unwrap_or(&resolved),
                &resolved,
                &current,
                alias.or_else(|| model_registry::alias_for_model(&resolved)),
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
    id: &str,
    model: &str,
    current: &str,
    alias: Option<&str>,
    source: &str,
    effort: Option<&str>,
) {
    if items.iter().any(|item| item.id == id) {
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
        id: id.to_string(),
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

fn bool_setting_label(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "not set".to_string())
}

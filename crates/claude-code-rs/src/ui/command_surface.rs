//! Interactive slash-command surfaces for the Rust TUI.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use serde_json::Value;

use crate::ipc::subsystem_handlers::{build_mcp_server_config_entries, build_mcp_server_info_list};
use crate::ipc::subsystem_types::LspRecommendationPayload;
use crate::types::app_state::AppState;

use super::agents::agents_list::AgentsListState;
use super::agents::types::{
    AgentDefinition, AgentMemoryScope as UiAgentMemoryScope, AgentSource, AgentSourceFilter,
};
use super::agents::utils::get_agent_source_display_name;
use super::diff::diff_dialog::{render_diff_dialog_lines, DiffDialogMode, DiffSource};
use super::diff::{DiffData, DiffFile, DiffStats};
use super::form_navigation::{FormOption, FormTab, TabbedFormEvent, TabbedFormState};
use super::hooks::hooks_config_menu::{HookConfigSummary, HooksConfigMenuState};
use super::hooks::select_event_mode::{HookEvent, HOOK_EVENTS};
use super::lsp_recommendation::lsp_recommendation_menu::{
    LspRecommendationDecision, LspRecommendationPromptState,
};
use super::mcp::index::{McpServer, McpServerKind, McpServerStatus, McpTool};
use super::mcp::mcp_list_panel::McpListPanelState;
use super::memory::memory_file_selector::{
    MemoryFileKind, MemoryFileOption, MemoryFileSelectorState,
};
use super::skills::skills_menu::{render_skills_menu, SkillMenuItem};
use super::skills_helpers::{skill_description, skill_display_name};
use super::tasks::background_tasks_dialog::render_background_tasks_dialog;
use super::tasks::{TaskKind as UiTaskKind, TaskState as UiTaskState, TaskStatus as UiTaskStatus};
use super::teams::teams_dialog::{render_teams_dialog, TeamSummary, TeammateStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSurfaceOutcome {
    None,
    Close,
    FillPrompt(String),
    Submit(String),
    LspRecommendationResponse {
        request_id: String,
        plugin_name: String,
        decision: String,
        install_prompt: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSurface {
    Agents(AgentsSurface),
    Config(ConfigSurface),
    Diff(DiffSurface),
    Hooks(HooksSurface),
    Mcp(McpSurface),
    Memory(MemorySurface),
    Sandbox(SandboxSurface),
    Skills(SkillsSurface),
    Tasks(TasksSurface),
    Team(TeamSurface),
    LspRecommendation(LspRecommendationSurface),
}

impl CommandSurface {
    pub fn for_slash_command(name: &str, args: &str, state: &AppState, cwd: &Path) -> Option<Self> {
        if !args.trim().is_empty() {
            return None;
        }

        match name {
            "agents" => Some(Self::Agents(AgentsSurface::new(cwd))),
            "config" => Some(Self::Config(ConfigSurface::new(state))),
            "diff" => Some(Self::Diff(DiffSurface::new(cwd))),
            "hooks" => Some(Self::Hooks(HooksSurface::new(&state.hooks))),
            "mcp" => Some(Self::Mcp(McpSurface::new(cwd))),
            "memory" => Some(Self::Memory(MemorySurface::new(cwd))),
            "sandbox" => Some(Self::Sandbox(SandboxSurface::new(state))),
            "skills" => Some(Self::Skills(SkillsSurface::new())),
            "tasks" => Some(Self::Tasks(TasksSurface::new())),
            "team" | "teams" => Some(Self::Team(TeamSurface::new(state))),
            _ => None,
        }
    }

    pub fn lsp_recommendation(payload: LspRecommendationPayload) -> Self {
        Self::LspRecommendation(LspRecommendationSurface {
            request_id: payload.request_id,
            state: LspRecommendationPromptState::new(
                payload.plugin_name,
                payload.plugin_description,
                payload.file_extension,
            ),
        })
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Agents(_) => "Agents",
            Self::Config(_) => "Config",
            Self::Diff(_) => "Diff",
            Self::Hooks(_) => "Hooks",
            Self::Mcp(_) => "MCP",
            Self::Memory(_) => "Memory",
            Self::Sandbox(_) => "Sandbox",
            Self::Skills(_) => "Skills",
            Self::Tasks(_) => "Tasks",
            Self::Team(_) => "Team",
            Self::LspRecommendation(_) => "LSP Plugin Recommendation",
        }
    }

    pub fn render(&self) -> String {
        match self {
            Self::Agents(surface) => surface.render(),
            Self::Config(surface) => surface.render(),
            Self::Diff(surface) => surface.render(),
            Self::Hooks(surface) => surface.render(),
            Self::Mcp(surface) => surface.render(),
            Self::Memory(surface) => surface.render(),
            Self::Sandbox(surface) => surface.render(),
            Self::Skills(surface) => surface.render(),
            Self::Tasks(surface) => surface.render(),
            Self::Team(surface) => surface.render(),
            Self::LspRecommendation(surface) => surface.render(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        if key.kind != KeyEventKind::Press {
            return CommandSurfaceOutcome::None;
        }
        if matches!(key.code, KeyCode::Esc) {
            if let Self::LspRecommendation(surface) = self {
                return surface.cancel();
            }
            return CommandSurfaceOutcome::Close;
        }

        match self {
            Self::Agents(surface) => surface.handle_key(key),
            Self::Config(surface) => surface.handle_key(key),
            Self::Diff(surface) => surface.handle_key(key),
            Self::Hooks(surface) => surface.handle_key(key),
            Self::Mcp(surface) => surface.handle_key(key),
            Self::Memory(surface) => surface.handle_key(key),
            Self::Sandbox(surface) => surface.handle_key(key),
            Self::Skills(surface) => surface.handle_key(key),
            Self::Tasks(surface) => surface.handle_key(key),
            Self::Team(surface) => surface.handle_key(key),
            Self::LspRecommendation(surface) => surface.handle_key(key),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsSurface {
    state: AgentsListState,
    source_tabs: Vec<AgentSourceFilter>,
    source_index: usize,
}

impl AgentsSurface {
    fn new(cwd: &Path) -> Self {
        let agents: Vec<AgentDefinition> = crate::ipc::agent_settings::list_all_agents(cwd)
            .into_iter()
            .map(agent_entry_to_ui)
            .collect();
        let source_tabs = agent_source_tabs(&agents);
        let mut state = AgentsListState::new(AgentSourceFilter::All, agents);
        state.show_create_new = false;
        state.create_new_selected = false;
        Self {
            state,
            source_tabs,
            source_index: 0,
        }
    }

    fn render(&self) -> String {
        let labels = self
            .source_tabs
            .iter()
            .map(|source| get_agent_source_display_name(*source))
            .collect::<Vec<_>>();
        format!(
            "{}\n{}\n\nLeft/Right switch source tabs | Up/Down navigate | Enter select | Esc close",
            render_tabs(&labels, self.source_index),
            self.state.render()
        )
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                self.switch_source_tab(-1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') => {
                self.switch_source_tab(1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_prev();
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.state.move_next();
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self
                .state
                .selected_agent()
                .map(|agent| {
                    CommandSurfaceOutcome::Submit(format!("/agents show {}", agent.agent_type))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn switch_source_tab(&mut self, direction: isize) {
        self.source_index = cycle_index(self.source_index, self.source_tabs.len(), direction);
        if let Some(source) = self.source_tabs.get(self.source_index).copied() {
            self.state.source = source;
            self.state.selected_index = 0;
            self.state.create_new_selected = false;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSurface {
    state: TabbedFormState,
}

impl ConfigSurface {
    fn new(state: &AppState) -> Self {
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

    fn render(&self) -> String {
        self.state.render_lines().join("\n")
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsSurface {
    items: Vec<SkillMenuItem>,
    selected_index: usize,
    filter: String,
}

impl SkillsSurface {
    fn new() -> Self {
        let mut items = crate::skills::get_all_skills()
            .into_iter()
            .map(skill_menu_item)
            .collect::<Vec<_>>();
        items.sort_by(|a, b| a.name.cmp(&b.name));
        Self {
            items,
            selected_index: 0,
            filter: String::new(),
        }
    }

    fn render(&self) -> String {
        format!(
            "{}\n\nType to filter | Backspace edit filter | Enter details | r reload | d diagnostics | Esc close",
            render_skills_menu(&self.items, self.selected_index, &self.filter)
        )
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.move_selection(1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self
                .selected_item()
                .map(|item| CommandSurfaceOutcome::Submit(format!("/skills {}", item.name)))
                .unwrap_or(CommandSurfaceOutcome::None),
            KeyCode::Backspace => {
                self.filter.pop();
                self.selected_index = 0;
                CommandSurfaceOutcome::None
            }
            KeyCode::Char('r') if self.filter.is_empty() => {
                CommandSurfaceOutcome::Submit("/skills reload".to_string())
            }
            KeyCode::Char('d') if self.filter.is_empty() => {
                CommandSurfaceOutcome::Submit("/skills diagnostics".to_string())
            }
            KeyCode::Char(ch) if !ch.is_control() => {
                self.filter.push(ch);
                self.selected_index = 0;
                CommandSurfaceOutcome::None
            }
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn move_selection(&mut self, direction: isize) {
        let visible_len = self.visible_indices().len();
        self.selected_index = cycle_index(self.selected_index, visible_len, direction);
    }

    fn selected_item(&self) -> Option<&SkillMenuItem> {
        let visible_indices = self.visible_indices();
        visible_indices
            .get(self.selected_index)
            .and_then(|idx| self.items.get(*idx))
    }

    fn visible_indices(&self) -> Vec<usize> {
        let filter = self.filter.to_ascii_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter_map(|(idx, item)| {
                (filter.is_empty()
                    || item.name.to_ascii_lowercase().contains(&filter)
                    || item.description.to_ascii_lowercase().contains(&filter))
                .then_some(idx)
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasksSurface {
    items: Vec<TaskSurfaceItem>,
    selected_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskSurfaceItem {
    task: UiTaskStatus,
    source: TaskSurfaceSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TaskSurfaceSource {
    Tool,
    Team { teammate_name: String },
}

impl TasksSurface {
    fn new() -> Self {
        Self {
            items: task_surface_items(),
            selected_index: 0,
        }
    }

    fn render(&self) -> String {
        let tasks = self
            .items
            .iter()
            .map(|item| item.task.clone())
            .collect::<Vec<_>>();
        render_background_tasks_dialog(&tasks, self.selected_index)
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Up => {
                self.selected_index = cycle_index(self.selected_index, self.items.len(), -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.selected_index = cycle_index(self.selected_index, self.items.len(), 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self
                .selected_item()
                .map(|item| CommandSurfaceOutcome::Submit(format!("/tasks show {}", item.task.id)))
                .unwrap_or(CommandSurfaceOutcome::None),
            KeyCode::Char('k') | KeyCode::Char('s') => self.selected_stop_command(),
            KeyCode::Char('d') => self.selected_delete_command(),
            KeyCode::Char('r') => CommandSurfaceOutcome::Submit("/tasks".to_string()),
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn selected_item(&self) -> Option<&TaskSurfaceItem> {
        self.items.get(self.selected_index)
    }

    fn selected_stop_command(&self) -> CommandSurfaceOutcome {
        let Some(item) = self.selected_item() else {
            return CommandSurfaceOutcome::None;
        };
        match &item.source {
            TaskSurfaceSource::Tool => {
                CommandSurfaceOutcome::Submit(format!("/tasks stop {}", item.task.id))
            }
            TaskSurfaceSource::Team { teammate_name } => {
                CommandSurfaceOutcome::Submit(format!("/team kill {}", teammate_name))
            }
        }
    }

    fn selected_delete_command(&self) -> CommandSurfaceOutcome {
        let Some(item) = self.selected_item() else {
            return CommandSurfaceOutcome::None;
        };
        match item.source {
            TaskSurfaceSource::Tool => {
                CommandSurfaceOutcome::Submit(format!("/tasks delete {}", item.task.id))
            }
            TaskSurfaceSource::Team { .. } => CommandSurfaceOutcome::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSurface {
    summary: TeamSummary,
    active: bool,
    selected_index: usize,
}

impl TeamSurface {
    fn new(state: &AppState) -> Self {
        let (summary, active) = team_summary_from_state(state);
        Self {
            summary,
            active,
            selected_index: 0,
        }
    }

    fn render(&self) -> String {
        let mut lines = render_teams_dialog(&self.summary, self.selected_index)
            .lines()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let help = if self.active {
            "Enter status | k kill | s send | p spawn | l list | c create | Esc close"
        } else {
            "c create | l list | Esc close"
        };
        if lines
            .last()
            .is_some_and(|line| line.starts_with("k kill |"))
        {
            if let Some(last) = lines.last_mut() {
                *last = help.to_string();
            }
        } else {
            lines.push(help.to_string());
        }
        lines.join("\n")
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Up => {
                self.selected_index =
                    cycle_index(self.selected_index, self.summary.teammates.len(), -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.selected_index =
                    cycle_index(self.selected_index, self.summary.teammates.len(), 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => CommandSurfaceOutcome::Submit("/team status".to_string()),
            KeyCode::Char('l') => CommandSurfaceOutcome::Submit("/team list".to_string()),
            KeyCode::Char('c') => CommandSurfaceOutcome::FillPrompt("/team create ".to_string()),
            KeyCode::Char('p') => CommandSurfaceOutcome::FillPrompt("/team spawn ".to_string()),
            KeyCode::Char('s') => self
                .selected_teammate()
                .map(|teammate| {
                    CommandSurfaceOutcome::FillPrompt(format!("/team send {} ", teammate.name))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            KeyCode::Char('k') => self
                .selected_teammate()
                .map(|teammate| {
                    CommandSurfaceOutcome::Submit(format!("/team kill {}", teammate.name))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn selected_teammate(&self) -> Option<&TeammateStatus> {
        self.summary.teammates.get(self.selected_index)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSurface {
    sources: Vec<DiffSource>,
    source_index: usize,
    selected_index: usize,
    mode: DiffDialogMode,
    error: Option<String>,
}

impl DiffSurface {
    fn new(cwd: &Path) -> Self {
        match build_diff_sources(cwd) {
            Ok(sources) => Self {
                sources,
                source_index: 0,
                selected_index: 0,
                mode: DiffDialogMode::List,
                error: None,
            },
            Err(error) => Self {
                sources: vec![DiffSource::current()],
                source_index: 0,
                selected_index: 0,
                mode: DiffDialogMode::List,
                error: Some(error),
            },
        }
    }

    fn render(&self) -> String {
        if let Some(error) = &self.error {
            return format!("Diff\n{error}\n\nEsc close");
        }
        render_diff_dialog_lines(
            "Uncommitted changes",
            self.sources
                .get(self.source_index)
                .map(|source| source.label.as_str()),
            &self.sources,
            self.source_index,
            self.selected_index,
            self.mode,
            100,
        )
        .join("\n")
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') if self.mode == DiffDialogMode::List => {
                self.switch_source(-1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') if self.mode == DiffDialogMode::List => {
                self.switch_source(1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_file(-1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.move_file(1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter
                if self.mode == DiffDialogMode::List && self.current_file_count() > 0 =>
            {
                self.mode = DiffDialogMode::Detail;
                CommandSurfaceOutcome::None
            }
            KeyCode::Char('b') if self.mode == DiffDialogMode::Detail => {
                self.mode = DiffDialogMode::List;
                CommandSurfaceOutcome::None
            }
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn switch_source(&mut self, direction: isize) {
        self.source_index = cycle_index(self.source_index, self.sources.len(), direction);
        self.selected_index = 0;
    }

    fn move_file(&mut self, direction: isize) {
        self.selected_index =
            cycle_index(self.selected_index, self.current_file_count(), direction);
    }

    fn current_file_count(&self) -> usize {
        self.sources
            .get(self.source_index)
            .map(|source| source.data.files.len())
            .unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksSurface {
    state: HooksConfigMenuState,
    scope_index: usize,
}

impl HooksSurface {
    fn new(hooks: &HashMap<String, Value>) -> Self {
        let mut items: Vec<HookConfigSummary> = HOOK_EVENTS
            .iter()
            .copied()
            .map(|event| hook_summary(event, hooks.get(event.label())))
            .collect();
        items.sort_by_key(|item| hook_event_order(item.event));
        Self {
            state: HooksConfigMenuState::new(items),
            scope_index: 0,
        }
    }

    fn render(&self) -> String {
        format!(
            "{}\n{}\n\nLeft/Right switch settings scope | Up/Down navigate | Enter select | o open scope | Esc close",
            render_tabs(&["User settings", "Project settings"], self.scope_index),
            self.state.render()
        )
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                self.scope_index = cycle_index(self.scope_index, 2, -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') => {
                self.scope_index = cycle_index(self.scope_index, 2, 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_prev();
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.state.move_next();
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self
                .state
                .selected_event()
                .map(|event| {
                    CommandSurfaceOutcome::Submit(format!("/hooks list {}", event.label()))
                })
                .unwrap_or(CommandSurfaceOutcome::None),
            KeyCode::Char('u') => CommandSurfaceOutcome::Submit("/hooks open user".to_string()),
            KeyCode::Char('p') => CommandSurfaceOutcome::Submit("/hooks open project".to_string()),
            KeyCode::Char('o') => CommandSurfaceOutcome::Submit(format!(
                "/hooks open {}",
                if self.scope_index == 0 {
                    "user"
                } else {
                    "project"
                }
            )),
            _ => CommandSurfaceOutcome::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpSurface {
    state: McpListPanelState,
    action_index: usize,
}

impl McpSurface {
    fn new(cwd: &Path) -> Self {
        Self {
            state: McpListPanelState::new(build_mcp_servers(cwd)),
            action_index: 0,
        }
    }

    fn render(&self) -> String {
        format!(
            "{}\n{}\n\nLeft/Right switch action tabs | Up/Down navigate | Enter select | a add | Esc close",
            render_tabs(&["Status", "Edit", "Reconnect", "Remove"], self.action_index),
            self.state.render()
        )
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                self.action_index = cycle_index(self.action_index, 4, -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') => {
                self.action_index = cycle_index(self.action_index, 4, 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_prev();
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.state.move_next();
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self.selected_mcp_action(),
            KeyCode::Char('a') => CommandSurfaceOutcome::FillPrompt("/mcp add ".to_string()),
            KeyCode::Char('e') => selected_server_command(&self.state, "/mcp edit ", " "),
            KeyCode::Char('r') => selected_server_command(&self.state, "/mcp reconnect ", ""),
            KeyCode::Char('d') => selected_server_command(&self.state, "/mcp remove ", ""),
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn selected_mcp_action(&self) -> CommandSurfaceOutcome {
        match self.action_index {
            0 => CommandSurfaceOutcome::Submit("/mcp status".to_string()),
            1 => selected_server_command(&self.state, "/mcp edit ", " "),
            2 => selected_server_command(&self.state, "/mcp reconnect ", ""),
            3 => selected_server_command(&self.state, "/mcp remove ", ""),
            _ => CommandSurfaceOutcome::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySurface {
    state: MemoryFileSelectorState,
    cwd: PathBuf,
    home: PathBuf,
    action_index: usize,
}

impl MemorySurface {
    fn new(cwd: &Path) -> Self {
        let home = crate::config::paths::data_root();
        Self {
            state: MemoryFileSelectorState::new(memory_options(cwd, &home)),
            cwd: cwd.to_path_buf(),
            home,
            action_index: 0,
        }
    }

    fn render(&self) -> String {
        format!(
            "{}\nMemory files\n{}\n\nLeft/Right switch action tabs | Up/Down navigate | Enter select | Esc close",
            render_tabs(&["Edit", "Show", "Paths", "Open"], self.action_index),
            self.state.render(&self.cwd, &self.home)
        )
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') => {
                self.action_index = cycle_index(self.action_index, 4, -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right | KeyCode::Char(']') => {
                self.action_index = cycle_index(self.action_index, 4, 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.move_prev();
                CommandSurfaceOutcome::None
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                self.state.move_next();
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self.selected_memory_action(),
            KeyCode::Char('s') => CommandSurfaceOutcome::Submit("/memory show".to_string()),
            KeyCode::Char('p') => CommandSurfaceOutcome::Submit("/memory path".to_string()),
            KeyCode::Char('a') => CommandSurfaceOutcome::Submit("/memory open auto".to_string()),
            KeyCode::Char('t') => CommandSurfaceOutcome::Submit("/memory open team".to_string()),
            KeyCode::Char('g') => CommandSurfaceOutcome::Submit("/memory open global".to_string()),
            KeyCode::Char('o') => CommandSurfaceOutcome::Submit("/memory open project".to_string()),
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn selected_memory_action(&self) -> CommandSurfaceOutcome {
        match self.action_index {
            0 => CommandSurfaceOutcome::Submit("/memory edit".to_string()),
            1 => CommandSurfaceOutcome::Submit("/memory show".to_string()),
            2 => CommandSurfaceOutcome::Submit("/memory path".to_string()),
            3 => selected_memory_open_command(&self.state),
            _ => CommandSurfaceOutcome::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxSurface {
    state: TabbedFormState,
}

impl SandboxSurface {
    fn new(state: &AppState) -> Self {
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
                        vec![FormOption::new("status", "Show sandbox status")
                            .with_description(format!("sandbox={enabled}; mode={mode}"))],
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

    fn render(&self) -> String {
        self.state.render_lines().join("\n")
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRecommendationSurface {
    request_id: String,
    state: LspRecommendationPromptState,
}

impl LspRecommendationSurface {
    fn render(&self) -> String {
        self.state.render()
    }

    fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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

    fn cancel(&self) -> CommandSurfaceOutcome {
        self.response(LspRecommendationDecision::No)
    }

    fn response(&self, decision: LspRecommendationDecision) -> CommandSurfaceOutcome {
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

fn skill_menu_item(skill: crate::skills::SkillDefinition) -> SkillMenuItem {
    SkillMenuItem {
        name: skill_display_name(&skill).to_string(),
        description: skill_description(&skill).to_string(),
        enabled: skill.is_user_invocable(),
        source: skill_source_label(&skill.source),
    }
}

fn skill_source_label(source: &crate::skills::SkillSource) -> String {
    match source {
        crate::skills::SkillSource::Bundled => "bundled".to_string(),
        crate::skills::SkillSource::User => "user".to_string(),
        crate::skills::SkillSource::Project => "project".to_string(),
        crate::skills::SkillSource::Plugin(name) => format!("plugin:{name}"),
        crate::skills::SkillSource::Mcp(name) => format!("mcp:{name}"),
    }
}

fn task_surface_items() -> Vec<TaskSurfaceItem> {
    let mut items = crate::tools::tasks::global_store()
        .list()
        .into_iter()
        .map(tool_task_surface_item)
        .collect::<Vec<_>>();
    items.extend(
        crate::teams::in_process::InProcessBackend::task_snapshots()
            .into_iter()
            .map(team_task_surface_item),
    );
    items
}

fn tool_task_surface_item(task: crate::tools::tasks::TaskEntry) -> TaskSurfaceItem {
    let title = if task.subject.trim().is_empty() {
        task.id.clone()
    } else {
        task.subject.clone()
    };
    let summary = first_non_empty([
        task.output_summary.as_str(),
        task.description.as_str(),
        task.status.as_str(),
    ]);
    let elapsed_ms = elapsed_ms_since_timestamp(task.created_at);
    let output_lines = task
        .output
        .lines()
        .take(20)
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    TaskSurfaceItem {
        task: UiTaskStatus {
            id: task.id,
            title,
            kind: ui_task_kind_from_tool_kind(&task.kind),
            state: ui_task_state_from_tool_status(task.status),
            progress: None,
            summary,
            elapsed_ms,
            output_lines,
        },
        source: TaskSurfaceSource::Tool,
    }
}

fn team_task_surface_item(task: crate::teams::in_process::TeammateTaskSnapshot) -> TaskSurfaceItem {
    let summary = if task.has_error {
        first_non_empty([
            task.error_message.as_deref().unwrap_or_default(),
            "team task failed",
        ])
    } else if task.awaiting_plan_approval {
        "awaiting plan approval".to_string()
    } else if task.is_idle {
        "idle".to_string()
    } else {
        first_non_empty([task.prompt.as_str(), "working"])
    };

    TaskSurfaceItem {
        task: UiTaskStatus {
            id: task.id,
            title: format!("{} ({})", task.agent_name, task.team_name),
            kind: UiTaskKind::InProcessTeammate,
            state: ui_task_state_from_team_status(task.status, task.has_error),
            progress: None,
            summary,
            elapsed_ms: 0,
            output_lines: vec![task.prompt.clone()],
        },
        source: TaskSurfaceSource::Team {
            teammate_name: task.agent_name,
        },
    }
}

fn ui_task_kind_from_tool_kind(kind: &str) -> UiTaskKind {
    match kind.to_ascii_lowercase().as_str() {
        value if value.contains("agent") => UiTaskKind::AsyncAgent,
        value if value.contains("remote") => UiTaskKind::RemoteSession,
        value if value.contains("monitor") => UiTaskKind::MonitorMcp,
        value if value.contains("dream") => UiTaskKind::Dream,
        value if value.contains("workflow") => UiTaskKind::Workflow,
        _ => UiTaskKind::Shell,
    }
}

fn ui_task_state_from_tool_status(status: crate::tools::tasks::TaskStatus) -> UiTaskState {
    match status {
        crate::tools::tasks::TaskStatus::Pending => UiTaskState::Pending,
        crate::tools::tasks::TaskStatus::InProgress
        | crate::tools::tasks::TaskStatus::Interrupted
        | crate::tools::tasks::TaskStatus::Recoverable => UiTaskState::Running,
        crate::tools::tasks::TaskStatus::Completed => UiTaskState::Succeeded,
        crate::tools::tasks::TaskStatus::Failed => UiTaskState::Failed,
        crate::tools::tasks::TaskStatus::Cancelled | crate::tools::tasks::TaskStatus::Stopped => {
            UiTaskState::Canceled
        }
    }
}

fn ui_task_state_from_team_status(
    status: crate::teams::types::TaskStatus,
    has_error: bool,
) -> UiTaskState {
    if has_error {
        return UiTaskState::Failed;
    }
    match status {
        crate::teams::types::TaskStatus::Running => UiTaskState::Running,
        crate::teams::types::TaskStatus::Stopped => UiTaskState::Canceled,
        crate::teams::types::TaskStatus::Completed => UiTaskState::Succeeded,
    }
}

fn team_summary_from_state(state: &AppState) -> (TeamSummary, bool) {
    let Some(context) = state
        .team_context
        .as_ref()
        .filter(|ctx| !ctx.team_name.is_empty())
    else {
        return (
            TeamSummary {
                name: "No active team".to_string(),
                teammates: Vec::new(),
            },
            false,
        );
    };

    let summary = crate::teams::helpers::read_team_file(&context.team_name)
        .map(|team_file| {
            let snapshots = crate::teams::in_process::InProcessBackend::task_snapshots();
            let teammates = team_file
                .members
                .iter()
                .filter(|member| member.name != crate::teams::constants::TEAM_LEAD_NAME)
                .map(|member| {
                    let matching = snapshots
                        .iter()
                        .filter(|snapshot| snapshot.agent_id == member.agent_id)
                        .collect::<Vec<_>>();
                    let mut status = TeammateStatus::new(
                        member.name.clone(),
                        first_non_empty([
                            member.prompt.as_deref().unwrap_or_default(),
                            member.agent_type.as_deref().unwrap_or("teammate"),
                        ]),
                    );
                    status.mode = member.mode.clone().unwrap_or_else(|| "ask".to_string());
                    status.state = teammate_state_label(member.is_active, &matching);
                    status.assigned_tasks = matching.len();
                    status.hidden = !member.tmux_pane_id.is_empty()
                        && team_file.hidden_pane_ids.contains(&member.tmux_pane_id);
                    status
                })
                .collect::<Vec<_>>();
            TeamSummary {
                name: team_file.name,
                teammates,
            }
        })
        .unwrap_or_else(|_| {
            let mut teammates = context
                .teammates
                .values()
                .map(|info| {
                    let mut status = TeammateStatus::new(
                        info.name.clone(),
                        info.agent_type.as_deref().unwrap_or("teammate"),
                    );
                    status.hidden = false;
                    status
                })
                .collect::<Vec<_>>();
            teammates.sort_by(|a, b| a.name.cmp(&b.name));
            TeamSummary {
                name: context.team_name.clone(),
                teammates,
            }
        });

    (summary, true)
}

fn teammate_state_label(
    active: Option<bool>,
    snapshots: &[&crate::teams::in_process::TeammateTaskSnapshot],
) -> String {
    if snapshots
        .iter()
        .any(|snapshot| snapshot.has_error || snapshot.error_message.is_some())
    {
        return "error".to_string();
    }
    if snapshots
        .iter()
        .any(|snapshot| snapshot.status == crate::teams::types::TaskStatus::Running)
    {
        if snapshots.iter().all(|snapshot| snapshot.is_idle) {
            return "idle".to_string();
        }
        return "running".to_string();
    }
    match active {
        Some(true) => "active".to_string(),
        Some(false) => "stopped".to_string(),
        None => "unknown".to_string(),
    }
}

fn build_diff_sources(cwd: &Path) -> Result<Vec<DiffSource>, String> {
    let repo = git2::Repository::discover(cwd)
        .map_err(|error| format!("Not a git repository (or any parent): {error}"))?;
    let staged = staged_diff_data(&repo).map_err(|error| format!("Failed staged diff: {error}"))?;
    let unstaged =
        unstaged_diff_data(&repo).map_err(|error| format!("Failed unstaged diff: {error}"))?;

    let mut sources = Vec::new();
    if !diff_data_is_empty(&staged) {
        sources.push(DiffSource::with_label("Staged", staged));
    }
    if !diff_data_is_empty(&unstaged) {
        sources.push(DiffSource::with_label("Unstaged", unstaged));
    }
    if sources.is_empty() {
        sources.push(DiffSource::with_label("Current", DiffData::empty()));
    }
    Ok(sources)
}

fn staged_diff_data(repo: &git2::Repository) -> Result<DiffData, git2::Error> {
    let head_tree = match repo.head() {
        Ok(head) => head
            .peel_to_commit()
            .ok()
            .and_then(|commit| commit.tree().ok()),
        Err(_) => None,
    };
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, None)?;
    diff_to_diff_data(&diff)
}

fn unstaged_diff_data(repo: &git2::Repository) -> Result<DiffData, git2::Error> {
    let diff = repo.diff_index_to_workdir(None, None)?;
    diff_to_diff_data(&diff)
}

fn diff_to_diff_data(diff: &git2::Diff<'_>) -> Result<DiffData, git2::Error> {
    let stats = diff
        .stats()
        .ok()
        .map(|stats| DiffStats::new(stats.files_changed(), stats.insertions(), stats.deletions()));
    let mut files = Vec::new();
    let mut indices = HashMap::new();
    for delta in diff.deltas() {
        ensure_diff_file(&mut files, &mut indices, &delta);
    }

    let mut hunks: HashMap<String, Vec<String>> = HashMap::new();
    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let path = ensure_diff_file(&mut files, &mut indices, &delta);
        if let Some(file) = files.get_mut(path.1) {
            match line.origin() {
                '+' => file.lines_added += 1,
                '-' => file.lines_removed += 1,
                _ => {}
            }
        }

        let mut rendered = String::new();
        if matches!(line.origin(), '+' | '-' | ' ') {
            rendered.push(line.origin());
        }
        if let Ok(content) = std::str::from_utf8(line.content()) {
            rendered.push_str(content.trim_end_matches('\n'));
        }
        if !rendered.trim().is_empty() {
            hunks.entry(path.0).or_default().push(rendered);
        }
        true
    })?;

    Ok(DiffData {
        stats,
        files,
        hunks,
        loading: false,
    })
}

fn ensure_diff_file(
    files: &mut Vec<DiffFile>,
    indices: &mut HashMap<String, usize>,
    delta: &git2::DiffDelta<'_>,
) -> (String, usize) {
    let path = diff_delta_path(delta);
    if let Some(idx) = indices.get(&path).copied() {
        return (path, idx);
    }
    let is_untracked = delta.status() == git2::Delta::Untracked;
    let idx = files.len();
    files.push(DiffFile::new(
        path.clone(),
        0,
        0,
        false,
        false,
        false,
        is_untracked,
    ));
    indices.insert(path.clone(), idx);
    (path, idx)
}

fn diff_delta_path(delta: &git2::DiffDelta<'_>) -> String {
    delta
        .new_file()
        .path()
        .or_else(|| delta.old_file().path())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "(unknown)".to_string())
}

fn diff_data_is_empty(data: &DiffData) -> bool {
    data.files.is_empty()
        && data
            .stats
            .as_ref()
            .map(|stats| stats.files_count == 0)
            .unwrap_or(true)
}

fn first_non_empty<const N: usize>(values: [&str; N]) -> String {
    values
        .into_iter()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .unwrap_or_default()
        .to_string()
}

fn elapsed_ms_since_timestamp(timestamp: i64) -> u64 {
    chrono::Utc::now()
        .timestamp()
        .saturating_sub(timestamp)
        .max(0) as u64
        * 1000
}

fn agent_entry_to_ui(entry: crate::ipc::subsystem_types::AgentDefinitionEntry) -> AgentDefinition {
    let mut agent = AgentDefinition::new(
        entry.name,
        entry.description,
        entry.system_prompt,
        match entry.source {
            crate::ipc::subsystem_types::AgentDefinitionSource::Builtin => AgentSource::BuiltIn,
            crate::ipc::subsystem_types::AgentDefinitionSource::User => AgentSource::User,
            crate::ipc::subsystem_types::AgentDefinitionSource::Project => AgentSource::Project,
            crate::ipc::subsystem_types::AgentDefinitionSource::Plugin { .. } => {
                AgentSource::Plugin
            }
        },
    );
    if !entry.tools.is_empty() {
        agent.tools = Some(entry.tools);
    }
    agent.color = entry.color;
    agent.model = entry.model;
    agent.filename = entry.filename;
    agent.base_dir = entry.file_path;
    agent.memory = entry.memory.map(|memory| match memory {
        crate::ipc::subsystem_types::AgentMemoryScope::User => UiAgentMemoryScope::User,
        crate::ipc::subsystem_types::AgentMemoryScope::Project => UiAgentMemoryScope::Project,
        crate::ipc::subsystem_types::AgentMemoryScope::Local => UiAgentMemoryScope::Local,
    });
    agent.effort = entry.effort;
    agent.permission_mode = entry.permission_mode.map(|mode| format!("{mode:?}"));
    agent.skills = entry.skills;
    agent
}

fn hook_summary(event: HookEvent, value: Option<&Value>) -> HookConfigSummary {
    let Some(configs) = value.and_then(Value::as_array) else {
        return HookConfigSummary {
            event,
            matcher_count: 0,
            command_count: 0,
        };
    };

    let command_count = configs
        .iter()
        .filter_map(|config| config.get("hooks"))
        .filter_map(Value::as_array)
        .map(Vec::len)
        .sum();
    HookConfigSummary {
        event,
        matcher_count: configs.len(),
        command_count,
    }
}

fn hook_event_order(event: HookEvent) -> usize {
    HOOK_EVENTS
        .iter()
        .position(|candidate| *candidate == event)
        .unwrap_or(usize::MAX)
}

fn build_mcp_servers(cwd: &Path) -> Vec<McpServer> {
    let entries = build_mcp_server_config_entries(cwd);
    let status = build_mcp_server_info_list();
    entries
        .into_iter()
        .map(|entry| {
            let live = status.iter().find(|item| item.name == entry.name);
            let kind = match entry.transport.as_str() {
                "sse" | "streamable-http" => McpServerKind::Remote,
                _ => McpServerKind::Stdio,
            };
            let mut server = McpServer::new(entry.name.clone(), kind);
            server.status = if entry.disabled.unwrap_or(false) {
                McpServerStatus::Disabled
            } else {
                live.map(|item| status_from_label(&item.state))
                    .unwrap_or(McpServerStatus::Connecting)
            };
            server.command_or_url = entry
                .url
                .or(entry.command)
                .unwrap_or_else(|| entry.scope.label());
            let tools_count = live.map(|item| item.tools_count).unwrap_or(0);
            server.tools = (0..tools_count)
                .map(|idx| McpTool::new(format!("tool-{}", idx + 1), "registered MCP tool"))
                .collect();
            if let Some(error) = live.and_then(|item| item.error.clone()) {
                server.warnings.push(error);
            }
            server
        })
        .collect()
}

fn status_from_label(value: &str) -> McpServerStatus {
    match value {
        "connected" | "running" => McpServerStatus::Connected,
        "connecting" | "starting" => McpServerStatus::Connecting,
        "disabled" | "disconnected" | "stopped" => McpServerStatus::Disabled,
        _ => McpServerStatus::Failed,
    }
}

fn selected_server_command(
    state: &McpListPanelState,
    prefix: &str,
    suffix: &str,
) -> CommandSurfaceOutcome {
    state
        .selected_server()
        .map(|server| {
            if prefix.contains(" edit ") {
                CommandSurfaceOutcome::FillPrompt(format!("{prefix}{}{suffix}", server.name))
            } else {
                CommandSurfaceOutcome::Submit(format!("{prefix}{}{suffix}", server.name))
            }
        })
        .unwrap_or(CommandSurfaceOutcome::None)
}

fn selected_memory_open_command(state: &MemoryFileSelectorState) -> CommandSurfaceOutcome {
    let Some(option) = state.options.get(state.selected_index) else {
        return CommandSurfaceOutcome::None;
    };
    if option.kind != MemoryFileKind::Folder {
        return CommandSurfaceOutcome::Submit("/memory edit".to_string());
    }

    let description = option.description.to_ascii_lowercase();
    let scope = if description.contains("auto") {
        "auto"
    } else if description.contains("team") {
        "team"
    } else if description.contains("global") {
        "global"
    } else {
        "project"
    };
    CommandSurfaceOutcome::Submit(format!("/memory open {scope}"))
}

fn memory_options(cwd: &Path, home: &Path) -> Vec<MemoryFileOption> {
    let mut options = vec![
        file_option(home.join("CLAUDE.md"), MemoryFileKind::User),
        file_option(cwd.join("CLAUDE.md"), MemoryFileKind::Project),
    ];
    for path in crate::config::claude_md::find_claude_md_files(cwd) {
        if path != cwd.join("CLAUDE.md") {
            options.push(file_option(path, MemoryFileKind::Nested));
        }
    }
    options.extend([
        MemoryFileOption::new(
            crate::config::paths::auto_memory_dir(),
            MemoryFileKind::Folder,
        )
        .with_description("auto-memory folder"),
        MemoryFileOption::new(
            crate::config::paths::team_memory_dir(cwd),
            MemoryFileKind::Folder,
        )
        .with_description("team-memory folder"),
        MemoryFileOption::new(
            crate::config::paths::memory_dir_global(),
            MemoryFileKind::Folder,
        )
        .with_description("global memory folder"),
        MemoryFileOption::new(cwd.join(".cc-rust").join("memory"), MemoryFileKind::Folder)
            .with_description("project memory folder"),
    ]);
    options
}

fn file_option(path: PathBuf, kind: MemoryFileKind) -> MemoryFileOption {
    let exists = path.exists();
    let option = MemoryFileOption::new(path, kind);
    if exists {
        option
    } else {
        option.missing()
    }
}

fn agent_source_tabs(agents: &[AgentDefinition]) -> Vec<AgentSourceFilter> {
    let candidates = [
        AgentSourceFilter::All,
        AgentSourceFilter::BuiltIn,
        AgentSourceFilter::Plugin,
        AgentSourceFilter::Source(AgentSource::User),
        AgentSourceFilter::Source(AgentSource::Project),
        AgentSourceFilter::Source(AgentSource::Local),
        AgentSourceFilter::Source(AgentSource::Policy),
        AgentSourceFilter::Source(AgentSource::Flag),
    ];

    candidates
        .into_iter()
        .filter(|filter| {
            *filter == AgentSourceFilter::All
                || agents.iter().any(|agent| match filter {
                    AgentSourceFilter::All => true,
                    AgentSourceFilter::BuiltIn => agent.source == AgentSource::BuiltIn,
                    AgentSourceFilter::Plugin => agent.source == AgentSource::Plugin,
                    AgentSourceFilter::Source(source) => agent.source == *source,
                })
        })
        .collect()
}

fn render_tabs(labels: &[impl AsRef<str>], selected: usize) -> String {
    labels
        .iter()
        .enumerate()
        .map(|(idx, label)| {
            let label = label.as_ref();
            if idx == selected {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn cycle_index(current: usize, len: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if direction < 0 {
        if current == 0 {
            len - 1
        } else {
            current - 1
        }
    } else {
        (current + 1) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn agents_surface_submits_selected_agent_detail_command() {
        let mut surface = CommandSurface::Agents(AgentsSurface::new(Path::new(".")));
        assert!(surface.render().contains("[Agents]"));
        surface.handle_key(key(KeyCode::Right));
        assert!(surface.render().contains("[Built-in agents]"));
        assert!(surface.render().contains("general-purpose"));
        match surface.handle_key(key(KeyCode::Enter)) {
            CommandSurfaceOutcome::Submit(command) => assert!(command.starts_with("/agents show ")),
            other => panic!("expected selected agent detail command, got {other:?}"),
        }
    }

    #[test]
    fn hooks_surface_navigates_to_event_command() {
        let mut hooks = HashMap::new();
        hooks.insert(
            "PostToolUse".to_string(),
            serde_json::json!([{ "matcher": "*", "hooks": [{ "command": "cargo test" }] }]),
        );
        let mut surface = CommandSurface::Hooks(HooksSurface::new(&hooks));
        surface.handle_key(key(KeyCode::Down));
        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::Submit("/hooks list PostToolUse".to_string())
        );
        surface.handle_key(key(KeyCode::Right));
        assert_eq!(
            surface.handle_key(key(KeyCode::Char('o'))),
            CommandSurfaceOutcome::Submit("/hooks open project".to_string())
        );
    }

    #[test]
    fn slash_command_surfaces_open_only_for_empty_interactive_commands() {
        let state = AppState::default();
        let cwd = std::env::current_dir().expect("current dir");

        for command in [
            "agents", "config", "diff", "hooks", "mcp", "memory", "sandbox", "skills", "tasks",
            "team",
        ] {
            assert!(
                CommandSurface::for_slash_command(command, "", &state, &cwd).is_some(),
                "{command} should open a command surface"
            );
            assert!(
                CommandSurface::for_slash_command(command, "status", &state, &cwd).is_none(),
                "{command} with args should keep the normal slash-command path"
            );
        }
    }

    #[test]
    fn skills_surface_filters_and_opens_selected_skill() {
        let mut surface = CommandSurface::Skills(SkillsSurface {
            items: vec![
                SkillMenuItem::new("debug", "diagnose failures"),
                SkillMenuItem::new("remember", "save memory"),
            ],
            selected_index: 0,
            filter: String::new(),
        });

        surface.handle_key(key(KeyCode::Char('m')));
        assert!(surface.render().contains("remember"));
        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::Submit("/skills remember".to_string())
        );
    }

    #[test]
    fn tasks_surface_routes_selected_task_actions() {
        let mut surface = CommandSurface::Tasks(TasksSurface {
            items: vec![TaskSurfaceItem {
                task: UiTaskStatus::new("task-1", "cargo test", UiTaskKind::Shell),
                source: TaskSurfaceSource::Tool,
            }],
            selected_index: 0,
        });

        assert!(surface.render().contains("Background tasks"));
        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::Submit("/tasks show task-1".to_string())
        );
        assert_eq!(
            surface.handle_key(key(KeyCode::Char('d'))),
            CommandSurfaceOutcome::Submit("/tasks delete task-1".to_string())
        );
    }

    #[test]
    fn team_surface_routes_teammate_actions() {
        let mut surface = CommandSurface::Team(TeamSurface {
            summary: TeamSummary {
                name: "ui-port".to_string(),
                teammates: vec![TeammateStatus::new("builder", "Implement changes")],
            },
            active: true,
            selected_index: 0,
        });

        assert!(surface.render().contains("Team: ui-port"));
        assert_eq!(
            surface.handle_key(key(KeyCode::Char('s'))),
            CommandSurfaceOutcome::FillPrompt("/team send builder ".to_string())
        );
        assert_eq!(
            surface.handle_key(key(KeyCode::Char('k'))),
            CommandSurfaceOutcome::Submit("/team kill builder".to_string())
        );
    }

    #[test]
    fn diff_surface_opens_selected_file_detail() {
        let source = DiffSource::with_label(
            "Current",
            DiffData {
                stats: Some(DiffStats::new(1, 1, 0)),
                files: vec![DiffFile::new(
                    "src/main.rs",
                    1,
                    0,
                    false,
                    false,
                    false,
                    false,
                )],
                hunks: HashMap::from([(
                    "src/main.rs".to_string(),
                    vec!["+fn main() {}".to_string()],
                )]),
                loading: false,
            },
        );
        let mut surface = CommandSurface::Diff(DiffSurface {
            sources: vec![source],
            source_index: 0,
            selected_index: 0,
            mode: DiffDialogMode::List,
            error: None,
        });

        assert!(surface.render().contains("src/main.rs"));
        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::None
        );
        assert!(surface.render().contains("+fn main() {}"));
    }

    #[test]
    fn config_surface_uses_tab_navigation_and_selection() {
        let mut surface = CommandSurface::Config(ConfigSurface::new(&AppState::default()));
        assert!(surface.render().contains("[Status]"));

        surface.handle_key(key(KeyCode::Right));
        assert!(surface.render().contains("[Config]"));
        surface.handle_key(key(KeyCode::Down));

        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::Submit("/config schema".to_string())
        );
        assert_eq!(
            surface.handle_key(key(KeyCode::Esc)),
            CommandSurfaceOutcome::Close
        );
    }

    #[test]
    fn sandbox_surface_uses_tab_navigation_and_selection() {
        let mut surface = CommandSurface::Sandbox(SandboxSurface::new(&AppState::default()));

        surface.handle_key(key(KeyCode::Right));
        surface.handle_key(key(KeyCode::Right));
        assert!(surface.render().contains("[Network]"));

        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::Submit("/sandbox network on".to_string())
        );
    }

    #[test]
    fn mcp_surface_action_tabs_apply_to_selected_server() {
        let mut server = McpServer::new("db", McpServerKind::Stdio);
        server.command_or_url = "node db-server.js".to_string();
        let mut surface = CommandSurface::Mcp(McpSurface {
            state: McpListPanelState::new(vec![server]),
            action_index: 0,
        });

        surface.handle_key(key(KeyCode::Right));
        assert!(surface.render().contains("[Edit]"));
        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::FillPrompt("/mcp edit db ".to_string())
        );

        surface.handle_key(key(KeyCode::Right));
        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::Submit("/mcp reconnect db".to_string())
        );
    }

    #[test]
    fn memory_surface_action_tabs_apply_to_selected_target() {
        let cwd = PathBuf::from(".");
        let home = PathBuf::from(".");
        let mut surface = CommandSurface::Memory(MemorySurface {
            state: MemoryFileSelectorState::new(vec![
                MemoryFileOption::new("CLAUDE.md", MemoryFileKind::Project),
                MemoryFileOption::new(".cc-rust/auto-memory", MemoryFileKind::Folder)
                    .with_description("auto-memory folder"),
            ]),
            cwd,
            home,
            action_index: 0,
        });

        surface.handle_key(key(KeyCode::Right));
        surface.handle_key(key(KeyCode::Right));
        surface.handle_key(key(KeyCode::Right));
        assert!(surface.render().contains("[Open]"));
        surface.handle_key(key(KeyCode::Down));

        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::Submit("/memory open auto".to_string())
        );
    }

    #[test]
    fn lsp_recommendation_yes_populates_plugin_install_prompt() {
        let mut surface = CommandSurface::lsp_recommendation(LspRecommendationPayload {
            request_id: "req-1".to_string(),
            plugin_name: "rust-analyzer".to_string(),
            plugin_description: None,
            file_extension: ".rs".to_string(),
            language_id: Some("rust".to_string()),
        });
        assert!(surface.render().contains("Yes, install rust-analyzer"));
        assert_eq!(
            surface.handle_key(key(KeyCode::Enter)),
            CommandSurfaceOutcome::LspRecommendationResponse {
                request_id: "req-1".to_string(),
                plugin_name: "rust-analyzer".to_string(),
                decision: "yes".to_string(),
                install_prompt: Some("/plugin install rust-analyzer ".to_string()),
            }
        );
    }
}

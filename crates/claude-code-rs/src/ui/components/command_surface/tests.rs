use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

use super::surfaces::tasks::{TaskSurfaceItem, TaskSurfaceSource};
use super::*;
use crate::ipc::subsystem_types::LspRecommendationPayload;
use crate::types::app_state::AppState;
use crate::ui::diff::diff_dialog::{DiffDialogMode, DiffSource};
use crate::ui::diff::{DiffData, DiffFile, DiffStats};
use crate::ui::mcp::index::{McpServer, McpServerKind};
use crate::ui::mcp::mcp_list_panel::McpListPanelState;
use crate::ui::memory::memory_file_selector::{
    MemoryFileKind, MemoryFileOption, MemoryFileSelectorState,
};
use crate::ui::skills::skills_menu::SkillMenuItem;
use crate::ui::tasks::{TaskKind as UiTaskKind, TaskStatus as UiTaskStatus};
use crate::ui::teams::teams_dialog::{TeamSummary, TeammateStatus};

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
        "agents", "config", "diff", "hooks", "mcp", "memory", "sandbox", "skills", "tasks", "team",
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
            hunks: HashMap::from([("src/main.rs".to_string(), vec!["+fn main() {}".to_string()])]),
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

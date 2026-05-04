use super::workspace_trust::trusted_workspaces_path;
use super::*;
use crate::types::message::{MessageContent, UserMessage};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serial_test::serial;
use std::path::Path;

struct EnvGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvGuard {
    fn set_path(key: &'static str, value: &Path) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.as_ref() {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

#[test]
fn render_places_prompt_after_compact_welcome() {
    let mut app = App::new();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");

    let content = buffer_to_lines(terminal.backend().buffer(), 80, 24);
    assert!(
        content[8].trim_start().starts_with(">"),
        "prompt should sit directly under the 8-line welcome panel"
    );
    assert!(
        !content[22].trim_start().starts_with(">"),
        "prompt should not be pinned to the bottom row"
    );
}

#[test]
fn render_places_prompt_after_short_chat_content() {
    let mut app = App::new();
    app.add_message(Message::User(UserMessage {
        uuid: uuid::Uuid::new_v4(),
        timestamp: 0,
        role: "user".to_string(),
        content: MessageContent::Text("hello".to_string()),
        is_meta: false,
        tool_use_result: None,
        source_tool_assistant_uuid: None,
    }));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");

    let content = buffer_to_lines(terminal.backend().buffer(), 80, 24);
    assert!(content[0].contains("You: hello"));
    assert!(
        content[1].trim_start().starts_with(">"),
        "prompt should follow the rendered chat content"
    );
    assert!(
        !content[22].trim_start().starts_with(">"),
        "prompt should not be pinned to the bottom row"
    );
}

#[test]
fn slash_opens_command_palette_and_selection_keeps_argument_entry() {
    let mut app = App::new();

    assert_eq!(send_key(&mut app, KeyCode::Char('/')), AppAction::None);
    assert!(app.command_palette.active());

    assert_eq!(send_key(&mut app, KeyCode::Char('m')), AppAction::None);
    assert_eq!(send_key(&mut app, KeyCode::Char('c')), AppAction::None);
    assert_eq!(send_key(&mut app, KeyCode::Enter), AppAction::None);

    assert_eq!(app.prompt.input, "/mcp ");
    assert!(!app.command_palette.active());
    assert!(
        CommandPalette::argument_hint(&app.prompt.input, std::path::Path::new(&app.cwd)).is_some()
    );
}

#[test]
fn ctrl_e_opens_edit_target_picker_for_supported_commands() {
    let mut app = App::new();
    app.prompt.input = "/mcp".to_string();
    app.prompt.cursor_position = app.prompt.input.len();
    app.sync_command_palette();

    assert_eq!(
        send_key_with_modifiers(&mut app, KeyCode::Char('e'), KeyModifiers::CONTROL),
        AppAction::None
    );
    assert!(app.command_palette.active());
    assert!(app.command_palette.edit_target_picker_active());
    assert_eq!(
        app.command_palette.selected_edit_target_input().as_deref(),
        Some("edit user")
    );
}

#[test]
fn picker_enter_inserts_selected_target_into_prompt() {
    let mut app = App::new();
    app.prompt.input = "/mcp".to_string();
    app.prompt.cursor_position = app.prompt.input.len();
    app.sync_command_palette();

    assert_eq!(
        send_key_with_modifiers(&mut app, KeyCode::Char('e'), KeyModifiers::CONTROL),
        AppAction::None
    );
    assert_eq!(send_key(&mut app, KeyCode::Enter), AppAction::None);

    assert_eq!(app.prompt.input, "/mcp edit user");
    assert!(!app.command_palette.edit_target_picker_active());
}

#[test]
fn command_palette_renders_below_prompt_input() {
    let mut app = App::new();
    app.prompt.input = "/".to_string();
    app.prompt.cursor_position = app.prompt.input.len();
    app.sync_command_palette();
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");

    let content = buffer_to_lines(terminal.backend().buffer(), 100, 24);
    let prompt_row = content
        .iter()
        .position(|line| line.trim_start().starts_with("> /"))
        .expect("prompt row");
    let commands_row = content
        .iter()
        .position(|line| line.contains(" Commands "))
        .expect("commands row");
    assert!(
        commands_row > prompt_row,
        "commands palette should render below the prompt input"
    );
}

#[test]
#[serial]
fn argument_entry_renders_parameter_help_near_input() {
    let mut app = App::new();
    app.prompt.input = "/plugin ".to_string();
    app.prompt.cursor_position = app.prompt.input.len();
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");

    let content = buffer_to_lines(terminal.backend().buffer(), 100, 24).join("\n");
    assert!(content.contains("/plugin arguments"));
    assert!(content.contains(
        "Usage: /plugin <list|installed|disabled|errors|status|enable|disable|uninstall> [id]"
    ));
    assert!(content.contains("installed_plugins.json"));
    assert!(
        content.contains("~/.cc-rust")
            || content.contains("$CC_RUST_HOME")
            || content.contains("file:///"),
        "expected an editable plugin path, got:\n{content}"
    );
}

#[test]
#[serial]
fn render_workspace_trust_prompt_after_cwd_is_set() {
    let home = tempfile::tempdir().expect("cc-rust home");
    let _home_guard = EnvGuard::set_path("CC_RUST_HOME", home.path());
    let workspace = tempfile::tempdir().expect("workspace");
    let cwd = workspace.path().display().to_string();

    let mut app = App::new();
    app.set_cwd(cwd);
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");

    let content = buffer_to_lines(terminal.backend().buffer(), 100, 24).join("\n");
    assert!(content.contains("Accessing workspace:"));
    assert!(content.contains("Yes, I trust this folder"));
    assert!(content.contains("No, exit"));
}

#[test]
#[serial]
fn workspace_trust_prompt_accepts_persists_and_exits() {
    let home = tempfile::tempdir().expect("cc-rust home");
    let _home_guard = EnvGuard::set_path("CC_RUST_HOME", home.path());
    let workspace = tempfile::tempdir().expect("workspace");
    let cwd = workspace.path().display().to_string();

    let mut app = App::new();
    app.set_cwd(cwd.clone());
    assert!(app.workspace_trust_pending);

    assert_eq!(send_key(&mut app, KeyCode::Enter), AppAction::None);
    assert!(!app.workspace_trust_pending);
    assert!(trusted_workspaces_path().exists());

    let mut reopened = App::new();
    reopened.set_cwd(cwd);
    assert!(!reopened.workspace_trust_pending);

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
    terminal.draw(|frame| reopened.render(frame)).expect("draw");
    let content = buffer_to_lines(terminal.backend().buffer(), 80, 24).join("\n");
    assert!(content.contains("Claude Code"));
    assert!(!content.contains("Quick safety check"));

    let other_workspace = tempfile::tempdir().expect("other workspace");
    let mut app = App::new();
    app.set_cwd(other_workspace.path().display().to_string());
    assert!(app.workspace_trust_pending);
    assert_eq!(send_key(&mut app, KeyCode::Esc), AppAction::Quit);
    assert!(app.should_quit());
}

#[test]
fn mouse_wheel_scrolls_prompt_messages() {
    let mut app = App::new();
    app.scroll_offset = 10;
    app.dirty = false;

    assert_eq!(
        send_mouse(&mut app, MouseEventKind::ScrollUp),
        AppAction::ScrollUp
    );
    assert_eq!(app.scroll_offset, 9);
    assert!(app.dirty);

    app.dirty = false;
    assert_eq!(
        send_mouse(&mut app, MouseEventKind::ScrollDown),
        AppAction::ScrollDown
    );
    assert_eq!(app.scroll_offset, 10);
    assert!(app.dirty);
}

#[test]
fn mouse_wheel_scrolls_transcript_view() {
    let mut app = App::new();
    app.view_mode = ViewMode::Transcript;
    app.transcript_state.scroll_offset = 10;

    assert_eq!(
        send_mouse(&mut app, MouseEventKind::ScrollUp),
        AppAction::ScrollUp
    );
    assert_eq!(app.transcript_state.scroll_offset, 9);

    assert_eq!(
        send_mouse(&mut app, MouseEventKind::ScrollDown),
        AppAction::ScrollDown
    );
    assert_eq!(app.transcript_state.scroll_offset, 10);
}

#[test]
fn transcript_arrow_keys_scroll_instead_of_history_fallback() {
    let mut app = App::new();
    app.view_mode = ViewMode::Transcript;
    app.transcript_state.scroll_offset = 10;

    assert_eq!(send_key(&mut app, KeyCode::Up), AppAction::None);
    assert_eq!(app.transcript_state.scroll_offset, 9);

    assert_eq!(send_key(&mut app, KeyCode::Down), AppAction::None);
    assert_eq!(app.transcript_state.scroll_offset, 10);
}

#[test]
fn prompt_arrow_keys_still_drive_history() {
    let mut app = App::new();
    app.push_history("first".to_string());
    app.push_history("second".to_string());

    assert_eq!(send_key(&mut app, KeyCode::Up), AppAction::None);
    assert_eq!(app.prompt.input, "second");

    assert_eq!(send_key(&mut app, KeyCode::Up), AppAction::None);
    assert_eq!(app.prompt.input, "first");

    assert_eq!(send_key(&mut app, KeyCode::Down), AppAction::None);
    assert_eq!(app.prompt.input, "second");
}

#[test]
fn ctrl_r_opens_history_search_and_escape_closes() {
    let mut app = App::new();
    app.push_history("first prompt".to_string());

    assert_eq!(
        send_key_with_modifiers(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL),
        AppAction::None
    );
    assert!(app.history_search_active());

    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");
    terminal.draw(|frame| app.render(frame)).expect("draw");
    let content = buffer_to_lines(terminal.backend().buffer(), 100, 24).join("\n");
    assert!(content.contains("Search prompts"));
    assert!(content.contains("first prompt"));

    assert_eq!(send_key(&mut app, KeyCode::Esc), AppAction::None);
    assert!(!app.history_search_active());
    assert!(app.prompt.input.is_empty());
}

#[test]
fn history_search_enter_fills_selected_prompt() {
    let mut app = App::new();
    app.push_history("first prompt".to_string());
    app.push_history("second prompt".to_string());

    assert_eq!(
        send_key_with_modifiers(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL),
        AppAction::None
    );
    assert_eq!(send_key(&mut app, KeyCode::Enter), AppAction::None);

    assert!(!app.history_search_active());
    assert_eq!(app.prompt.input, "second prompt");
    assert_eq!(app.prompt.cursor_position, app.prompt.input.len());
}

#[test]
fn history_search_filters_before_selecting() {
    let mut app = App::new();
    app.push_history("git status --short".to_string());
    app.push_history("cargo test -p claude-code-rs history_search".to_string());

    assert_eq!(
        send_key_with_modifiers(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL),
        AppAction::None
    );
    assert_eq!(send_key(&mut app, KeyCode::Char('g')), AppAction::None);
    assert_eq!(send_key(&mut app, KeyCode::Char('i')), AppAction::None);
    assert_eq!(send_key(&mut app, KeyCode::Char('t')), AppAction::None);
    assert_eq!(send_key(&mut app, KeyCode::Enter), AppAction::None);

    assert_eq!(app.prompt.input, "git status --short");
}

#[test]
fn command_surface_handles_selection_before_prompt_input() {
    let mut app = App::new();
    app.open_command_surface(CommandSurface::lsp_recommendation(
        lsp_recommendation_payload(None),
    ));

    assert_eq!(
        send_key(&mut app, KeyCode::Enter),
        AppAction::LspRecommendationResponse {
            request_id: "req-1".to_string(),
            plugin_name: "rust-analyzer".to_string(),
            decision: "yes".to_string(),
        }
    );
    assert_eq!(app.prompt.input, "/plugin install rust-analyzer ");
    assert!(!app.command_surface_active());
}

#[test]
fn command_surface_renders_as_overlay() {
    let mut app = App::new();
    app.open_command_surface(CommandSurface::lsp_recommendation(
        lsp_recommendation_payload(Some("Rust language server".to_string())),
    ));
    let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("terminal");

    terminal.draw(|frame| app.render(frame)).expect("draw");

    let content = buffer_to_lines(terminal.backend().buffer(), 100, 24).join("\n");
    assert!(content.contains("LSP Plugin Recommendation"));
    assert!(content.contains("Yes, install rust-analyzer"));
}

fn send_key(app: &mut App, code: KeyCode) -> AppAction {
    app.handle_key_event(KeyEvent::new(code, KeyModifiers::NONE))
}

fn lsp_recommendation_payload(
    plugin_description: Option<String>,
) -> crate::ipc::subsystem_types::LspRecommendationPayload {
    crate::ipc::subsystem_types::LspRecommendationPayload {
        request_id: "req-1".to_string(),
        plugin_name: "rust-analyzer".to_string(),
        plugin_description,
        file_extension: ".rs".to_string(),
        language_id: Some("rust".to_string()),
    }
}

fn send_key_with_modifiers(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> AppAction {
    app.handle_key_event(KeyEvent::new(code, modifiers))
}

fn send_mouse(app: &mut App, kind: MouseEventKind) -> AppAction {
    app.handle_mouse_event(MouseEvent {
        kind,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    })
}

fn buffer_to_lines(buf: &ratatui::buffer::Buffer, width: u16, height: u16) -> Vec<String> {
    let mut lines = Vec::new();
    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            line.push_str(buf[(x, y)].symbol());
        }
        lines.push(line);
    }
    lines
}

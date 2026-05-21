pub mod agent_navigation;
mod agent_tree_dialog;
pub mod app_event;
pub mod app_event_sender;
mod input;
mod render;
pub mod status;
#[cfg(test)]
#[path = "app/tests.rs"]
mod tests;
mod transcript_mode;
mod voice;
mod workspace_trust;
use agent_navigation::{AgentNavigationState, AgentThreadEntry};
use agent_tree_dialog::AgentTreeDialog;
use cc_config::settings::StatusLineSettings;
use cc_ipc_protocol::BackendMessage;
use cc_keybindings::KeybindingRegistry;
use cc_services::prompt_suggestion::PromptSuggestion;
use cc_types::agent_events::{AgentEvent, TeamEvent};
use cc_types::callbacks::AskUserRequestPayload;
use cc_types::message::Message;
use cc_voice::VoiceController;
use ratatui::layout::Rect;
use status::SessionUsageSnapshot;
use workspace_trust::is_workspace_trusted;

use super::command_palette::CommandPalette;
use super::command_surface::CommandSurface;
use super::history_search_dialog::{HistorySearchDialog, HistorySearchEntry};
use super::notifications::in_app::{
    InAppNotification, NotificationPriority, NotificationState, NotificationTone,
};
use super::overlays::dialog::ExitGuard;
use super::permissions::worker_pending_permission::render_worker_pending_permission;
use super::permissions::{
    BypassPermissionsModeChoice, BypassPermissionsModeDialog, PermissionChoice, PermissionDialog,
    PermissionDialogRequest, QuestionDialog,
};
use super::prompt_input::PromptInput;
use super::spinner::SpinnerState;
use super::status_line::StatusLineRunner;
use super::terminal_env::TerminalEnvConfig;
use super::theme::{Theme, ThemeProvider};
use super::transcript::{TranscriptState, ViewMode};
use super::vim::VimState;
use super::virtual_scroll::VirtualScroll;
use app_event::AppEvent;

/// Actions produced by the app in response to user input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppAction {
    None,
    Submit(String),
    Abort,
    Quit,
    ScrollUp,
    ScrollDown,
    PermissionResponse(PermissionChoice),
    QuestionResponse(String),
    BypassPermissionsModeResponse(BypassPermissionsModeChoice),
    AgentThreadSelected(String),
    KillAgentThreads(Vec<String>),
    LspRecommendationResponse {
        request_id: String,
        plugin_name: String,
        decision: String,
    },
    /// Transcript mode requested an export to `$EDITOR`. Carries the
    /// pre-rendered markdown body; the caller writes it to disk and
    /// spawns the editor so `App` stays free of IO.
    ExportTranscript(String),
    CopyMessage(String),
}

#[derive(Debug, Clone, Copy)]
struct SessionScrollbarState {
    area: Rect,
    total_lines: usize,
}

fn notification_from_app_event(
    key: String,
    message: String,
    level: String,
    timeout_ms: Option<u64>,
) -> InAppNotification {
    let (priority, tone) = match level.to_ascii_lowercase().as_str() {
        "immediate" => (NotificationPriority::Immediate, NotificationTone::Error),
        "error" => (NotificationPriority::High, NotificationTone::Error),
        "warning" | "warn" => (NotificationPriority::High, NotificationTone::Warning),
        "high" => (NotificationPriority::High, NotificationTone::Info),
        "low" => (NotificationPriority::Low, NotificationTone::Dim),
        "dim" | "debug" => (NotificationPriority::Low, NotificationTone::Dim),
        "medium" | "info" | "notice" => (NotificationPriority::Medium, NotificationTone::Info),
        _ => (NotificationPriority::Medium, NotificationTone::Info),
    };
    let mut notification = InAppNotification::new(key, priority, message)
        .with_tone(tone)
        .with_fold(true);
    if let Some(timeout_ms) = timeout_ms {
        notification = notification.with_timeout_ms(timeout_ms);
    }
    notification
}

fn notification_from_backend_message(message: &BackendMessage) -> Option<InAppNotification> {
    match message {
        BackendMessage::NotificationSent { title, level } => Some(notification_from_app_event(
            "backend-notification".to_string(),
            title.clone(),
            level.clone(),
            Some(4500),
        )),
        BackendMessage::Error { message, .. } => Some(notification_from_app_event(
            "backend-error".to_string(),
            message.clone(),
            "error".to_string(),
            Some(8000),
        )),
        BackendMessage::HookPermissionDecision { event } => Some(
            InAppNotification::new(
                "hook-permission-decision",
                NotificationPriority::Low,
                format!(
                    "hook: {}\nmatcher: {}\ndecision: {}",
                    event.hook_name, event.matcher, event.decision
                ),
            )
            .with_tone(NotificationTone::Info)
            .with_fold(true)
            .with_timeout_ms(5000),
        ),
        BackendMessage::PermissionDecisionDebug { event } => Some(
            InAppNotification::new(
                "permission-decision-debug",
                NotificationPriority::Low,
                format!(
                    "tool: {}\nmatcher: {}\nsource: {}\nbehavior: {}",
                    event.tool_name, event.matcher, event.source, event.behavior
                ),
            )
            .with_tone(NotificationTone::Dim)
            .with_fold(true)
            .with_timeout_ms(5000),
        ),
        _ => None,
    }
}

/// Main TUI application state.
pub struct App {
    messages: Vec<Message>,
    selected_message: Option<usize>,
    selected_message_expanded: bool,
    prompt: PromptInput,
    scroll_offset: usize,
    is_streaming: bool,
    spinner_state: SpinnerState,
    bypass_permissions_mode_dialog: Option<BypassPermissionsModeDialog>,
    permission_dialog: Option<PermissionDialog>,
    question_dialog: Option<QuestionDialog>,
    exit_guard: ExitGuard,
    should_quit: bool,
    design_theme_provider: ThemeProvider,
    theme: Theme,
    model_name: String,
    backend_name: String,
    session_id: String,
    cwd: String,
    output_style: Option<String>,
    verbose: bool,
    permission_mode_label: String,
    sandbox_label: String,
    effort_label: Option<String>,
    remote_indicator_label: Option<String>,
    session_cost_usd: f64,
    /// Whether the welcome screen is currently shown.
    show_welcome: bool,
    /// Startup trust gate shown before the normal welcome panel.
    workspace_trust_pending: bool,
    workspace_trust_selection: usize,
    history: Vec<HistorySearchEntry>,
    history_index: Option<usize>,
    saved_input: String,
    command_palette: CommandPalette,
    command_surface: Option<CommandSurface>,
    history_search_dialog: Option<HistorySearchDialog>,
    agent_nav: AgentNavigationState,
    agent_tree_dialog: Option<AgentTreeDialog>,
    show_agent_footer: bool,
    current_agent_thread_id: Option<String>,

    // Prompt suggestions
    /// Next-prompt suggestions shown after an assistant turn completes.
    suggestions: Option<Vec<PromptSuggestion>>,
    notifications: NotificationState,

    // Optimizations
    /// Virtual scroll: per-message height cache + prefix-sum offsets.
    vscroll: VirtualScroll,
    /// Last rendered session scrollbar, used for mouse click/drag control.
    session_scrollbar: Option<SessionScrollbarState>,
    session_scrollbar_dragging: bool,
    /// Dirty flag; when false, the TUI skips `terminal.draw()`.
    dirty: bool,
    /// Tick counter for throttling spinner frame advances.
    tick_counter: u32,
    keybindings: KeybindingRegistry,
    pending_chord: Vec<cc_keybindings::keystroke::Keystroke>,
    vim: VimState,

    // Scriptable status line (issue #11)
    /// Resolved status-line configuration (command, padding, intervals).
    /// Populated from effective settings at TUI startup; further updates
    /// go through [`Self::update_status_line_settings`] when the user
    /// reloads config.
    status_line_settings: StatusLineSettings,
    /// Subprocess runner shared handle used by `/statusline` as well.
    status_line_runner: StatusLineRunner,
    /// Accumulated usage / cost for the current session (fed to the
    /// status-line payload). Updated from engine `Result` events.
    session_usage: SessionUsageSnapshot,

    // Transcript / focus view + terminal env (issue #12)
    /// Which view the user is currently in; cycled with `Ctrl+O`.
    view_mode: ViewMode,
    /// Extra state only used in transcript/focus modes.
    transcript_state: TranscriptState,
    /// Env-driven terminal config: `CLAUDE_CODE_NO_FLICKER`,
    /// `CLAUDE_CODE_ENABLE_MOUSE_CAPTURE`, `CLAUDE_CODE_DISABLE_MOUSE`,
    /// `CLAUDE_CODE_SCROLL_SPEED`.
    terminal_env: TerminalEnvConfig,

    // Voice dictation (issue #13)
    /// Push-to-talk controller. `None` until the TUI runner installs
    /// one via [`Self::set_voice_controller`]. Default construction
    /// (e.g. in tests) leaves it `None` so nothing accidentally records.
    voice: Option<VoiceController>,
    /// Snapshot of the effective `voiceEnabled` + `language` settings.
    /// Updated whenever `/voice` / `/config set` flips them so the
    /// push-to-talk key handler doesn't need to re-read AppState.
    voice_enabled: bool,
    /// Whether this build/runtime supports actual recording and STT.
    voice_supported: bool,
    /// Normalized STT language passed to the controller on press.
    voice_language: String,

    // Completion state (Lane E)
    /// Tracks the active completion session for the input prompt.
    completion_state: input::CompletionState,
}

impl App {
    pub fn new() -> Self {
        let design_theme_provider = ThemeProvider::from_user_settings();
        let theme = design_theme_provider.legacy_theme();
        Self {
            messages: Vec::new(),
            selected_message: None,
            selected_message_expanded: false,
            prompt: PromptInput::new(),
            scroll_offset: 0,
            is_streaming: false,
            spinner_state: SpinnerState::new(),
            bypass_permissions_mode_dialog: None,
            permission_dialog: None,
            question_dialog: None,
            exit_guard: ExitGuard::new(),
            should_quit: false,
            design_theme_provider,
            theme,
            model_name: String::new(),
            backend_name: String::new(),
            session_id: String::new(),
            cwd: String::new(),
            output_style: None,
            verbose: false,
            permission_mode_label: String::new(),
            sandbox_label: String::new(),
            effort_label: None,
            remote_indicator_label: None,
            session_cost_usd: 0.0,
            show_welcome: true,
            workspace_trust_pending: false,
            workspace_trust_selection: 0,
            suggestions: None,
            notifications: NotificationState::default(),
            history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            command_palette: CommandPalette::new(),
            command_surface: None,
            history_search_dialog: None,
            agent_nav: AgentNavigationState::default(),
            agent_tree_dialog: None,
            show_agent_footer: true,
            current_agent_thread_id: None,
            vscroll: VirtualScroll::new(),
            session_scrollbar: None,
            session_scrollbar_dragging: false,
            dirty: true,
            tick_counter: 0,
            keybindings: KeybindingRegistry::with_defaults(),
            pending_chord: Vec::new(),
            vim: VimState::new(),
            status_line_settings: StatusLineSettings::default(),
            status_line_runner: StatusLineRunner::new(),
            session_usage: SessionUsageSnapshot::default(),
            view_mode: ViewMode::default(),
            transcript_state: TranscriptState::default(),
            terminal_env: TerminalEnvConfig::default(),
            voice: None,
            voice_enabled: false,
            voice_supported: false,
            voice_language: "en".to_string(),
            completion_state: input::CompletionState::new(),
        }
    }

    // Dirty flag

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    // Public API

    pub fn add_message(&mut self, msg: Message) {
        // Dismiss welcome screen on first user or assistant message.
        if self.show_welcome && matches!(msg, Message::User(_) | Message::Assistant(_)) {
            self.show_welcome = false;
        }
        self.messages.push(msg);
        self.sync_primary_agent_thread();
        self.clamp_selected_message();
        self.vscroll
            .invalidate_from(self.messages.len().saturating_sub(1));
        self.scroll_to_bottom_deferred();
        self.dirty = true;
    }

    pub fn replace_last_message(&mut self, msg: Message) {
        if let Some(last) = self.messages.last_mut() {
            *last = msg;
        } else {
            self.messages.push(msg);
        }
        self.sync_primary_agent_thread();
        self.clamp_selected_message();
        self.vscroll
            .invalidate_from(self.messages.len().saturating_sub(1));
        self.scroll_to_bottom_deferred();
        self.dirty = true;
    }

    pub fn remove_last_message(&mut self) {
        if self.messages.pop().is_some() {
            self.sync_primary_agent_thread();
            self.clamp_selected_message();
            self.vscroll.invalidate_from(self.messages.len());
            self.scroll_to_bottom_deferred();
            self.dirty = true;
        }
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    #[cfg(test)]
    pub fn selected_message(&self) -> Option<usize> {
        self.selected_message
    }

    fn clamp_selected_message(&mut self) {
        if self.messages.is_empty() {
            self.selected_message = None;
            self.selected_message_expanded = false;
        } else if let Some(idx) = self.selected_message {
            self.selected_message = Some(idx.min(self.messages.len() - 1));
        }
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.selected_message = None;
        self.selected_message_expanded = false;
        self.scroll_offset = 0;
        self.agent_nav.clear();
        self.current_agent_thread_id = None;
        self.sync_primary_agent_thread();
        self.vscroll.invalidate_all();
        self.dirty = true;
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        if self.is_streaming != streaming {
            self.is_streaming = streaming;
            if streaming {
                self.spinner_state.start(Some("Thinking...".to_string()));
                self.prompt.is_active = false;
                self.suggestions = None; // clear stale suggestions
            } else {
                self.spinner_state.stop();
                self.prompt.is_active = true;
            }
            self.dirty = true;
        }
    }

    #[cfg(test)]
    pub fn show_permission_dialog(&mut self, tool_name: &str, input: &str, message: &str) {
        self.permission_dialog = Some(PermissionDialog::new(tool_name, input, message));
        self.dirty = true;
    }

    pub fn show_question_dialog(&mut self, id: impl Into<String>, request: AskUserRequestPayload) {
        self.question_dialog = Some(QuestionDialog::new(id, request));
        self.dirty = true;
    }

    pub fn show_permission_request(&mut self, request: PermissionDialogRequest) {
        self.permission_dialog = Some(if request.tool_use_id.is_empty() {
            let input = request.tool_input.to_string();
            PermissionDialog::new(&request.tool_name, &input, &request.message)
        } else {
            PermissionDialog::from_request(request)
        });
        self.dirty = true;
    }

    pub fn show_bypass_permissions_mode_dialog(&mut self, disabled: bool) {
        self.bypass_permissions_mode_dialog = Some(BypassPermissionsModeDialog::new(disabled));
        self.dirty = true;
    }

    #[cfg(test)]
    pub fn dismiss_permission_dialog(&mut self) {
        self.permission_dialog = None;
        self.dirty = true;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    /// Tick the spinner. Called at 16ms interval; spinner frame advances
    /// every 5th tick (~80ms) to keep a pleasant animation speed.
    pub fn tick(&mut self) {
        self.tick_counter = self.tick_counter.wrapping_add(1);
        if self.spinner_state.active && self.tick_counter.is_multiple_of(5) {
            self.spinner_state.tick();
            self.dirty = true;
        }
        if self.notifications.process_queue() {
            self.dirty = true;
        }
    }

    pub fn set_model_name(&mut self, name: String) {
        self.model_name = name;
        self.dirty = true;
    }

    pub fn set_backend_name(&mut self, name: String) {
        self.backend_name = name;
        self.dirty = true;
    }

    pub fn set_session_id(&mut self, id: String) {
        let previous_session_id = self.session_id.clone();
        self.session_id = id;
        if !previous_session_id.is_empty() && previous_session_id != self.session_id {
            self.agent_nav.remove(&previous_session_id);
        }
        self.sync_primary_agent_thread();
        self.dirty = true;
    }

    pub fn set_cwd(&mut self, cwd: String) {
        self.cwd = cwd;
        self.workspace_trust_pending = !self.cwd.is_empty() && !is_workspace_trusted(&self.cwd);
        self.workspace_trust_selection = 0;
        self.dirty = true;
    }

    pub fn set_output_style(&mut self, output_style: Option<String>) {
        self.output_style = output_style;
        self.dirty = true;
    }

    pub fn set_theme_setting(&mut self, theme: Option<&str>) {
        self.design_theme_provider = ThemeProvider::from_setting_str(theme);
        self.theme = self.design_theme_provider.legacy_theme();
        self.dirty = true;
    }

    pub fn set_keybindings(&mut self, keybindings: KeybindingRegistry) {
        self.keybindings = keybindings;
        self.pending_chord.clear();
    }

    pub fn set_editor_mode(&mut self, editor_mode: Option<&str>) {
        self.vim = VimState::from_editor_mode(editor_mode);
        self.dirty = true;
    }

    pub fn update_session_cost(&mut self, cost_usd: f64) {
        self.session_cost_usd = cost_usd;
        self.dirty = true;
    }

    // Terminal env + transcript (issue #12)

    /// Install a resolved terminal-env config. Called by the TUI runner
    /// once at startup; `ScrollSpeed` is then consumed by the scroll
    /// helpers in both prompt and transcript modes.
    pub fn set_terminal_env(&mut self, cfg: TerminalEnvConfig) {
        self.terminal_env = cfg;
        self.dirty = true;
    }

    /// Current terminal-env config (read by the TUI runner to decide
    /// whether to emit synchronized-update escapes).
    pub fn terminal_env(&self) -> TerminalEnvConfig {
        self.terminal_env
    }

    /// Current view mode; tests and the TUI key binding use this to
    /// verify Ctrl+O cycling.
    #[cfg(test)]
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// Advance the view mode (`Prompt -> Transcript -> Focus -> Prompt`).
    /// Exposed separately so `/` slash commands could also drive it in
    /// the future.
    pub fn cycle_view_mode(&mut self) {
        // Leaving transcript throws away search state; entering fresh
        // next time is less surprising than stale matches hanging around.
        if self.view_mode.is_transcript_like() {
            self.transcript_state.clear_search();
        }
        self.view_mode = self.view_mode.next();
        // Seed the transcript scroll offset to the bottom on entry so the
        // user starts reading the latest exchange.
        if self.view_mode.is_transcript_like() {
            self.transcript_state.scroll_offset = usize::MAX;
        }
        self.dirty = true;
    }

    /// Open a modal slash-command surface above the normal prompt.
    pub fn open_command_surface(&mut self, surface: CommandSurface) {
        self.command_surface = Some(surface);
        self.command_palette.close();
        self.dirty = true;
    }

    #[cfg(test)]
    pub fn command_surface_active(&self) -> bool {
        self.command_surface.is_some()
    }

    #[cfg(test)]
    pub fn history_search_active(&self) -> bool {
        self.history_search_dialog.is_some()
    }

    /// Current transcript state exposed read-only so tests can assert
    /// search invariants without going through the render path.
    #[cfg(test)]
    pub fn transcript_state(&self) -> &TranscriptState {
        &self.transcript_state
    }

    pub fn set_spinner_message(&mut self, msg: String) {
        self.spinner_state.set_message(msg);
        self.dirty = true;
    }

    pub fn set_suggestions(&mut self, suggestions: Vec<PromptSuggestion>) {
        self.suggestions = Some(suggestions);
        self.dirty = true;
    }

    pub fn add_notification(&mut self, notification: InAppNotification) {
        if self.notifications.add_notification(notification) {
            self.dirty = true;
        }
    }

    pub fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Notification {
                key,
                message,
                level,
                timeout_ms,
            } => {
                self.add_notification(notification_from_app_event(key, message, level, timeout_ms));
            }
            AppEvent::LocalNotice { message } => {
                self.add_notification(
                    InAppNotification::new("local-notice", NotificationPriority::Medium, message)
                        .with_timeout_ms(4500)
                        .with_fold(true),
                );
            }
            AppEvent::Tick => self.tick(),
            AppEvent::Shutdown => {
                self.should_quit = true;
                self.dirty = true;
            }
            AppEvent::Backend { message } => {
                self.update_agent_navigation_from_backend(&message);
                if let Some(notification) = notification_from_backend_message(&message) {
                    self.add_notification(notification);
                }
            }
        }
    }

    pub(super) fn agent_footer_visible(&self) -> bool {
        self.show_agent_footer
            && self.agent_nav.active_non_primary_count() > 0
            && self
                .agent_nav
                .entry(self.current_agent_thread_id())
                .is_some_and(|entry| !entry.is_primary && !entry.is_closed)
    }

    pub(super) fn active_agent_thread_ids(&self) -> Vec<String> {
        self.agent_nav.active_non_primary_thread_ids()
    }

    pub(super) fn current_agent_thread_id(&self) -> &str {
        self.current_agent_thread_id
            .as_deref()
            .filter(|id| self.agent_nav.contains_thread(id))
            .or_else(|| (!self.session_id.is_empty()).then_some(self.session_id.as_str()))
            .or_else(|| {
                self.agent_nav
                    .ordered_threads()
                    .first()
                    .map(|entry| entry.thread_id.as_str())
            })
            .unwrap_or("")
    }

    pub(super) fn current_agent_label(&self) -> Option<String> {
        self.agent_nav
            .active_agent_label(self.current_agent_thread_id())
            .or_else(|| self.agent_footer_visible().then_some("Primary".to_string()))
    }

    pub(super) fn toggle_agent_tree_dialog(&mut self) {
        if self.agent_tree_dialog.is_some() {
            self.agent_tree_dialog = None;
        } else if self.agent_nav.thread_count() > 1 {
            self.agent_tree_dialog = Some(AgentTreeDialog::from_state(
                &self.agent_nav,
                self.current_agent_thread_id(),
            ));
        }
        self.dirty = true;
    }

    fn sync_primary_agent_thread(&mut self) {
        if self.session_id.is_empty() {
            return;
        }
        self.agent_nav.upsert(AgentThreadEntry {
            thread_id: self.session_id.clone(),
            agent_nickname: Some("Primary".to_string()),
            agent_role: Some("main".to_string()),
            is_primary: true,
            is_closed: false,
        });

        if self
            .current_agent_thread_id
            .as_deref()
            .is_none_or(|id| !self.agent_nav.contains_thread(id))
        {
            self.current_agent_thread_id = Some(self.session_id.clone());
        }
    }

    fn update_agent_navigation_from_backend(&mut self, message: &BackendMessage) {
        match message {
            BackendMessage::AgentEvent { event } => self.apply_agent_event(event),
            BackendMessage::TeamEvent { event } => self.apply_team_event(event),
            _ => {}
        }
    }

    fn apply_agent_event(&mut self, event: &AgentEvent) {
        self.sync_primary_agent_thread();
        match event {
            AgentEvent::Spawned {
                agent_id,
                description,
                agent_type,
                ..
            } => {
                self.agent_nav.upsert(AgentThreadEntry {
                    thread_id: agent_id.clone(),
                    agent_nickname: short_agent_label(description, agent_id),
                    agent_role: agent_type.clone(),
                    is_primary: false,
                    is_closed: false,
                });
                self.current_agent_thread_id = Some(agent_id.clone());
            }
            AgentEvent::Completed { agent_id, .. }
            | AgentEvent::Error { agent_id, .. }
            | AgentEvent::Aborted { agent_id } => {
                self.agent_nav.mark_closed(agent_id);
                self.normalize_current_agent_thread();
            }
            AgentEvent::PermissionQueued {
                agent_id,
                tool_name,
                summary,
                queue_position,
                ..
            } => {
                if self.agent_nav.contains_thread(agent_id) {
                    self.current_agent_thread_id = Some(agent_id.clone());
                }
                self.add_notification(
                    InAppNotification::new(
                        "worker-permission",
                        NotificationPriority::Low,
                        render_worker_pending_permission(
                            self.agent_nav
                                .entry(agent_id)
                                .and_then(|entry| entry.agent_nickname.as_deref())
                                .unwrap_or(agent_id),
                            tool_name,
                            summary,
                            *queue_position,
                        ),
                    )
                    .with_tone(NotificationTone::Warning)
                    .with_fold(true)
                    .with_timeout_ms(5000),
                );
            }
            AgentEvent::PermissionResolved { agent_id, .. } => {
                if self.agent_nav.contains_thread(agent_id) {
                    self.current_agent_thread_id = Some(agent_id.clone());
                }
            }
            AgentEvent::TreeSnapshot { roots } => {
                let current = self.current_agent_thread_id().to_string();
                self.agent_nav.clear();
                self.sync_primary_agent_thread();
                for node in roots {
                    self.collect_agent_tree_node(node);
                }
                if self.agent_nav.contains_thread(&current) {
                    self.current_agent_thread_id = Some(current);
                }
            }
            AgentEvent::StreamDelta { agent_id, .. }
            | AgentEvent::ThinkingDelta { agent_id, .. }
            | AgentEvent::ToolUse { agent_id, .. }
            | AgentEvent::ToolResult { agent_id, .. } => {
                if self.agent_nav.contains_thread(agent_id) {
                    self.current_agent_thread_id = Some(agent_id.clone());
                }
            }
        }
        self.dirty = true;
    }

    fn apply_team_event(&mut self, event: &TeamEvent) {
        self.sync_primary_agent_thread();
        match event {
            TeamEvent::MemberJoined {
                agent_id,
                agent_name,
                role,
                ..
            } => {
                self.agent_nav.upsert(AgentThreadEntry {
                    thread_id: agent_id.clone(),
                    agent_nickname: Some(agent_name.clone()),
                    agent_role: role.clone(),
                    is_primary: false,
                    is_closed: false,
                });
            }
            TeamEvent::MemberLeft { agent_id, .. } => {
                self.agent_nav.mark_closed(agent_id);
                self.normalize_current_agent_thread();
            }
            TeamEvent::StatusSnapshot { members, .. } => {
                for member in members {
                    self.agent_nav.upsert(AgentThreadEntry {
                        thread_id: member.agent_id.clone(),
                        agent_nickname: Some(member.agent_name.clone()),
                        agent_role: member.role.clone(),
                        is_primary: false,
                        is_closed: !member.is_active,
                    });
                }
            }
            TeamEvent::MessageRouted { from, to, .. } => {
                if self.agent_nav.contains_thread(from) {
                    self.current_agent_thread_id = Some(from.clone());
                } else if self.agent_nav.contains_thread(to) {
                    self.current_agent_thread_id = Some(to.clone());
                }
            }
        }
        self.dirty = true;
    }

    fn normalize_current_agent_thread(&mut self) {
        let current_is_active = self
            .current_agent_thread_id
            .as_deref()
            .and_then(|id| self.agent_nav.entry(id))
            .is_some_and(|entry| !entry.is_closed);
        if current_is_active {
            return;
        }
        if !self.session_id.is_empty() && self.agent_nav.contains_thread(&self.session_id) {
            self.current_agent_thread_id = Some(self.session_id.clone());
            return;
        }
        self.current_agent_thread_id = self
            .agent_nav
            .ordered_threads()
            .into_iter()
            .find(|entry| !entry.is_closed)
            .map(|entry| entry.thread_id.clone());
    }

    fn collect_agent_tree_node(&mut self, node: &cc_types::agent_types::AgentNode) {
        self.agent_nav.upsert(AgentThreadEntry {
            thread_id: node.agent_id.clone(),
            agent_nickname: short_agent_label(&node.description, &node.agent_id),
            agent_role: node.agent_type.clone(),
            is_primary: false,
            is_closed: !agent_state_is_active(&node.state),
        });
        for child in &node.children {
            self.collect_agent_tree_node(child);
        }
    }

    pub fn remove_notification(&mut self, key: &str) {
        if self.notifications.remove_notification(key) {
            self.dirty = true;
        }
    }

    pub fn current_notification(&self) -> Option<&InAppNotification> {
        self.notifications.current()
    }

    #[cfg(test)]
    pub fn clear_suggestions(&mut self) {
        if self.suggestions.is_some() {
            self.suggestions = None;
            self.dirty = true;
        }
    }

    #[cfg(test)]
    pub fn suggestions(&self) -> Option<&[PromptSuggestion]> {
        self.suggestions.as_deref()
    }

    pub fn push_history(&mut self, text: String) {
        if self.history.last().map(|entry| entry.display.as_str()) != Some(text.as_str()) {
            self.history
                .push(HistorySearchEntry::new(text, current_unix_secs()));
        }
        self.history_index = None;
        self.saved_input.clear();
    }

    /// Seed Ctrl+R prompt history from backend session storage.
    ///
    /// Callers pass entries newest-first (the shape returned by the session
    /// reader). `App` keeps history oldest-first so arrow-history and Ctrl+R
    /// continue to prefer live prompts appended during this TUI run.
    pub fn seed_persistent_history(&mut self, entries: Vec<HistorySearchEntry>) {
        for entry in entries.into_iter().rev() {
            if self
                .history
                .iter()
                .any(|existing| existing.display == entry.display)
            {
                continue;
            }
            self.history.push(entry);
        }
        self.history_index = None;
        self.dirty = true;
    }

    #[cfg(test)]
    pub(crate) fn history_len(&self) -> usize {
        self.history.len()
    }

    // Event handling
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn current_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn short_agent_label(description: &str, fallback: &str) -> Option<String> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return Some(fallback.to_string());
    }
    let mut words = trimmed.split_whitespace();
    let first = words.next().unwrap_or_default();
    let second = words.next().unwrap_or_default();
    let label = if second.is_empty() {
        first.to_string()
    } else {
        format!("{first} {second}")
    };
    Some(label)
}

fn agent_state_is_active(state: &str) -> bool {
    matches!(
        state.to_ascii_lowercase().as_str(),
        "running" | "active" | "spawned"
    )
}

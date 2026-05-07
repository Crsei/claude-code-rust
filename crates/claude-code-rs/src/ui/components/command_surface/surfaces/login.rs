use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::command_surface::{CommandSurfaceOutcome, cycle_index, render_tabs};

const LOGIN_ACTIONS: &[LoginAction] = &[
    LoginAction {
        shortcut: 's',
        label: "Status",
        description: "show current authentication source",
        command: LoginCommand::Submit("/login status"),
    },
    LoginAction {
        shortcut: '1',
        label: "API key",
        description: "paste a full Anthropic API key",
        command: LoginCommand::FillPrompt("/login "),
    },
    LoginAction {
        shortcut: '2',
        label: "Claude.ai",
        description: "start Claude.ai OAuth for Pro/Max accounts",
        command: LoginCommand::Submit("/login 2"),
    },
    LoginAction {
        shortcut: '3',
        label: "Console",
        description: "start Anthropic Console OAuth for API billing",
        command: LoginCommand::Submit("/login 3"),
    },
    LoginAction {
        shortcut: '4',
        label: "Codex",
        description: "start OpenAI Codex OAuth for ChatGPT accounts",
        command: LoginCommand::Submit("/login 4"),
    },
    LoginAction {
        shortcut: '5',
        label: "Codex CLI",
        description: "check or import ~/.codex/auth.json",
        command: LoginCommand::Submit("/login 5"),
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSurface {
    pub(crate) action_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LoginAction {
    shortcut: char,
    label: &'static str,
    description: &'static str,
    command: LoginCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginCommand {
    FillPrompt(&'static str),
    Submit(&'static str),
}

impl LoginSurface {
    pub(crate) fn new() -> Self {
        Self { action_index: 0 }
    }

    pub(crate) fn render(&self) -> String {
        let labels = LOGIN_ACTIONS
            .iter()
            .map(|action| action.label)
            .collect::<Vec<_>>();
        format!(
            "{}\nLogin methods\n{}\n\nLeft/Right switch action tabs | Up/Down navigate | Enter select | s status | 1-5 select | Esc close",
            render_tabs(&labels, self.action_index),
            self.render_action_list()
        )
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match key.code {
            KeyCode::Left | KeyCode::Char('[') | KeyCode::Up | KeyCode::Char('k') => {
                self.action_index = cycle_index(self.action_index, LOGIN_ACTIONS.len(), -1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Right
            | KeyCode::Char(']')
            | KeyCode::Down
            | KeyCode::Char('j')
            | KeyCode::Tab => {
                self.action_index = cycle_index(self.action_index, LOGIN_ACTIONS.len(), 1);
                CommandSurfaceOutcome::None
            }
            KeyCode::Enter => self.selected_login_action(),
            KeyCode::Char(ch) => self.shortcut_action(ch),
            _ => CommandSurfaceOutcome::None,
        }
    }

    pub(crate) fn selected_login_action(&self) -> CommandSurfaceOutcome {
        let Some(action) = LOGIN_ACTIONS.get(self.action_index) else {
            return CommandSurfaceOutcome::None;
        };

        match action.command {
            LoginCommand::FillPrompt(command) => {
                CommandSurfaceOutcome::FillPrompt(command.to_string())
            }
            LoginCommand::Submit(command) => CommandSurfaceOutcome::Submit(command.to_string()),
        }
    }

    fn render_action_list(&self) -> String {
        LOGIN_ACTIONS
            .iter()
            .enumerate()
            .map(|(idx, action)| {
                let marker = if idx == self.action_index { ">" } else { " " };
                format!(
                    "{marker} {}. {:<10} - {}",
                    action.shortcut, action.label, action.description
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn shortcut_action(&mut self, shortcut: char) -> CommandSurfaceOutcome {
        let Some(index) = LOGIN_ACTIONS
            .iter()
            .position(|action| action.shortcut.eq_ignore_ascii_case(&shortcut))
        else {
            return CommandSurfaceOutcome::None;
        };

        self.action_index = index;
        self.selected_login_action()
    }
}

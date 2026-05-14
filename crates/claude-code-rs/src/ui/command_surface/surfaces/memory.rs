use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::command_surface::adapters::memory::{memory_options, selected_memory_open_command};
use crate::ui::command_surface::{cycle_index, render_tabs, CommandSurfaceOutcome};
use crate::ui::memory::memory_file_selector::MemoryFileSelectorState;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySurface {
    pub(crate) state: MemoryFileSelectorState,
    pub(crate) cwd: PathBuf,
    pub(crate) home: PathBuf,
    pub(crate) action_index: usize,
}

impl MemorySurface {
    pub(crate) fn new(cwd: &Path) -> Self {
        let home = cc_config::paths::data_root();
        Self {
            state: MemoryFileSelectorState::new(memory_options(cwd, &home)),
            cwd: cwd.to_path_buf(),
            home,
            action_index: 0,
        }
    }

    pub(crate) fn render(&self) -> String {
        format!(
            "{}\nMemory files\n{}\n\nLeft/Right switch action tabs | Up/Down navigate | Enter select | Esc close",
            render_tabs(&["Edit", "Show", "Paths", "Open"], self.action_index),
            self.state.render(&self.cwd, &self.home)
        )
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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

    pub(crate) fn selected_memory_action(&self) -> CommandSurfaceOutcome {
        match self.action_index {
            0 => CommandSurfaceOutcome::Submit("/memory edit".to_string()),
            1 => CommandSurfaceOutcome::Submit("/memory show".to_string()),
            2 => CommandSurfaceOutcome::Submit("/memory path".to_string()),
            3 => selected_memory_open_command(&self.state),
            _ => CommandSurfaceOutcome::None,
        }
    }
}

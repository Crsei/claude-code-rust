use std::path::Path;

use crossterm::event::KeyCode;

use super::selection_surface::{SelectionItem, SelectionSurface};

mod edit_targets;
mod filter;
mod metadata;
mod render;
#[cfg(test)]
mod tests;

use edit_targets::{has_edit_target_picker, EditTarget};
use filter::{command_from_argument_input, filtered_commands};
use render::{argument_edit_row_count, palette_detail_rows};

const MAX_ROWS: usize = 6;
const DETAIL_ROWS: u16 = 4;
const ARG_HELP_BASE_HEIGHT: u16 = 5;
const MAX_EDIT_ROWS: usize = 2;
const MAX_EDIT_TARGET_ROWS: usize = 4;

#[derive(Debug, Clone)]
pub struct CommandPalette {
    active: bool,
    query: String,
    selected: usize,
    filtered: Vec<CommandItem>,
    edit_target_picker: Option<SelectionSurface>,
}

#[derive(Debug, Clone)]
struct CommandItem {
    name: String,
    aliases: Vec<String>,
    description: String,
    usage: String,
    examples: Vec<String>,
    edit_targets: Vec<EditTarget>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            selected: 0,
            filtered: Vec::new(),
            edit_target_picker: None,
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn sync_from_input(&mut self, input: &str, cwd: &Path) {
        let Some(without_slash) = input.strip_prefix('/') else {
            self.close();
            return;
        };

        if without_slash.contains(char::is_whitespace) {
            self.close();
            return;
        }

        self.active = true;
        self.query = without_slash.to_string();
        self.filtered = filtered_commands(&self.query, cwd);
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
        self.edit_target_picker = None;
    }

    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.selected = 0;
        self.filtered.clear();
        self.edit_target_picker = None;
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        if !self.active {
            return false;
        }

        match code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                true
            }
            KeyCode::Down => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                }
                true
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(MAX_ROWS);
                true
            }
            KeyCode::PageDown => {
                if !self.filtered.is_empty() {
                    self.selected = (self.selected + MAX_ROWS).min(self.filtered.len() - 1);
                }
                true
            }
            KeyCode::Esc => {
                self.close();
                true
            }
            _ => false,
        }
    }

    pub fn selected_command_input(&self) -> Option<String> {
        self.filtered
            .get(self.selected)
            .map(|cmd| format!("/{} ", cmd.name))
    }

    pub fn selected_command_has_edit_targets(&self) -> bool {
        self.filtered
            .get(self.selected)
            .is_some_and(has_edit_target_picker)
    }

    pub fn edit_target_picker_active(&self) -> bool {
        self.edit_target_picker.is_some()
    }

    pub fn open_edit_target_picker(&mut self) -> bool {
        let Some(cmd) = self.filtered.get(self.selected) else {
            return false;
        };

        let items: Vec<SelectionItem> = cmd
            .edit_targets
            .iter()
            .filter(|target| !target.insert.is_empty())
            .map(|target| {
                let mut item = SelectionItem::new(target.insert.clone(), target.label.clone());
                item.description = target.display.clone();
                item
            })
            .collect();

        if items.is_empty() {
            return false;
        }

        self.edit_target_picker = Some(SelectionSurface::new(
            format!("Edit targets for /{}", cmd.name),
            items,
        ));
        true
    }

    pub fn close_edit_target_picker(&mut self) {
        self.edit_target_picker = None;
    }

    pub fn handle_edit_target_key(&mut self, code: KeyCode) -> bool {
        let Some(picker) = self.edit_target_picker.as_mut() else {
            return false;
        };

        match code {
            KeyCode::Up | KeyCode::Left => {
                picker.move_prev();
                true
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                picker.move_next();
                true
            }
            KeyCode::PageUp => {
                picker.move_prev();
                true
            }
            KeyCode::PageDown => {
                picker.move_next();
                true
            }
            KeyCode::Esc => {
                self.close_edit_target_picker();
                true
            }
            _ => false,
        }
    }

    pub fn selected_edit_target_input(&self) -> Option<String> {
        self.edit_target_picker
            .as_ref()
            .and_then(|picker| picker.selected_item())
            .map(|item| item.id.clone())
    }

    pub fn argument_hint(input: &str, cwd: &Path) -> Option<String> {
        command_from_argument_input(input, cwd).map(|item| item.usage)
    }

    pub fn argument_help_height(input: &str, cwd: &Path) -> u16 {
        command_from_argument_input(input, cwd)
            .map(|item| {
                ARG_HELP_BASE_HEIGHT
                    + argument_edit_row_count(&item) as u16
                    + has_edit_target_picker(&item) as u16
            })
            .unwrap_or(0)
    }

    pub fn preferred_height(&self) -> u16 {
        if !self.active || self.filtered.is_empty() {
            return 0;
        }
        let list_rows = self.filtered.len().min(MAX_ROWS);
        let detail_rows = self
            .filtered
            .get(self.selected)
            .map(|item| palette_detail_rows(item, usize::MAX))
            .unwrap_or(DETAIL_ROWS);
        let list_rows = if detail_rows > DETAIL_ROWS {
            list_rows.saturating_sub((detail_rows - DETAIL_ROWS) as usize)
        } else {
            list_rows
        } as u16;
        let picker_rows = self
            .edit_target_picker
            .as_ref()
            .map(|picker| picker.render_lines(MAX_EDIT_TARGET_ROWS).len() as u16)
            .unwrap_or(0);
        (list_rows + DETAIL_ROWS + 2 + picker_rows).min(16)
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

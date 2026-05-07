use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::command_surface::adapters::tasks::task_surface_items;
use crate::ui::command_surface::{CommandSurfaceOutcome, cycle_index};
use crate::ui::tasks::TaskStatus as UiTaskStatus;
use crate::ui::tasks::background_tasks_dialog::render_background_tasks_dialog;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasksSurface {
    pub(crate) items: Vec<TaskSurfaceItem>,
    pub(crate) selected_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TaskSurfaceItem {
    pub(crate) task: UiTaskStatus,
    pub(crate) source: TaskSurfaceSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TaskSurfaceSource {
    Tool,
    Team { teammate_name: String },
}

impl TasksSurface {
    pub(crate) fn new() -> Self {
        Self {
            items: task_surface_items(),
            selected_index: 0,
        }
    }

    pub(crate) fn render(&self) -> String {
        let tasks = self
            .items
            .iter()
            .map(|item| item.task.clone())
            .collect::<Vec<_>>();
        render_background_tasks_dialog(&tasks, self.selected_index)
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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

    pub(crate) fn selected_item(&self) -> Option<&TaskSurfaceItem> {
        self.items.get(self.selected_index)
    }

    pub(crate) fn selected_stop_command(&self) -> CommandSurfaceOutcome {
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

    pub(crate) fn selected_delete_command(&self) -> CommandSurfaceOutcome {
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

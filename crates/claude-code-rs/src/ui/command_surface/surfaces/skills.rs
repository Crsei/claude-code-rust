use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::command_surface::{cycle_index, CommandSurfaceOutcome};
use crate::ui::skills::skills_menu::{render_skills_menu, SkillMenuItem};
use crate::ui::skills_helpers::{skill_description, skill_display_name};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsSurface {
    pub(crate) items: Vec<SkillMenuItem>,
    pub(crate) selected_index: usize,
    pub(crate) filter: String,
}

impl SkillsSurface {
    pub(crate) fn new() -> Self {
        let mut items = cc_skills::get_all_skills()
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

    pub(crate) fn render(&self) -> String {
        format!(
            "{}\n\nType to filter | Backspace edit filter | Enter details | r reload | d diagnostics | Esc close",
            render_skills_menu(&self.items, self.selected_index, &self.filter)
        )
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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

    pub(crate) fn move_selection(&mut self, direction: isize) {
        let visible_len = self.visible_indices().len();
        self.selected_index = cycle_index(self.selected_index, visible_len, direction);
    }

    pub(crate) fn selected_item(&self) -> Option<&SkillMenuItem> {
        let visible_indices = self.visible_indices();
        visible_indices
            .get(self.selected_index)
            .and_then(|idx| self.items.get(*idx))
    }

    pub(crate) fn visible_indices(&self) -> Vec<usize> {
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

pub(crate) fn skill_menu_item(skill: cc_skills::SkillDefinition) -> SkillMenuItem {
    SkillMenuItem {
        name: skill_display_name(&skill).to_string(),
        description: skill_description(&skill).to_string(),
        enabled: skill.is_user_invocable(),
        source: skill_source_label(&skill.source),
    }
}

pub(crate) fn skill_source_label(source: &cc_skills::SkillSource) -> String {
    match source {
        cc_skills::SkillSource::Bundled => "bundled".to_string(),
        cc_skills::SkillSource::User => "user".to_string(),
        cc_skills::SkillSource::Project => "project".to_string(),
        cc_skills::SkillSource::Plugin(name) => format!("plugin:{name}"),
        cc_skills::SkillSource::Mcp(name) => format!("mcp:{name}"),
    }
}

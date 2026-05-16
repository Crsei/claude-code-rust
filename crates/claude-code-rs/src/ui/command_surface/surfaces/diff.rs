use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::better_view_panel::{selected_row, BetterViewPanel};
use crate::ui::command_surface::adapters::diff::build_diff_sources;
use crate::ui::command_surface::{cycle_index, CommandSurfaceOutcome};
use crate::ui::diff::diff_detail_view::render_diff_detail_view_lines;
use crate::ui::diff::diff_dialog::{DiffDialogMode, DiffSource};
use crate::ui::diff::diff_file_list::render_diff_file_list_lines;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffSurface {
    pub(crate) sources: Vec<DiffSource>,
    pub(crate) source_index: usize,
    pub(crate) selected_index: usize,
    pub(crate) mode: DiffDialogMode,
    pub(crate) error: Option<String>,
}

impl DiffSurface {
    pub(crate) fn new(cwd: &Path) -> Self {
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

    pub(crate) fn render(&self) -> String {
        if let Some(error) = &self.error {
            return format!("Diff\n{error}\n\nEsc close");
        }
        let Some(source) = self.sources.get(self.source_index) else {
            return BetterViewPanel::new("Uncommitted changes")
                .summary("source=none files=0")
                .sections_title("Sources")
                .sections(Vec::new(), 0)
                .detail_title("Files")
                .detail_lines(vec!["Working tree is clean".to_string()])
                .footer("Esc close")
                .render();
        };
        let stats = source
            .data
            .stats
            .as_ref()
            .map(|stats| {
                format!(
                    "files={} +{} -{}",
                    stats.files_count, stats.lines_added, stats.lines_removed
                )
            })
            .unwrap_or_else(|| format!("files={}", source.data.files.len()));
        let sections = self
            .sources
            .iter()
            .map(|source| source.label.clone())
            .collect::<Vec<_>>();
        match self.mode {
            DiffDialogMode::List => {
                let detail_lines = if source.data.files.is_empty() {
                    vec!["Working tree is clean".to_string()]
                } else {
                    render_diff_file_list_lines(&source.data.files, self.selected_index, 60)
                };
                BetterViewPanel::new("Uncommitted changes")
                    .summary(format!("source={} {}", source.label, stats))
                    .sections_title("Sources")
                    .sections(sections, self.source_index)
                    .detail_title("Files")
                    .detail_lines(detail_lines)
                    .footer("Left/Right source | Up/Down file | Enter detail | Esc close")
                    .render()
            }
            DiffDialogMode::Detail => {
                let selected = source.data.files.get(self.selected_index);
                let mut detail_lines = Vec::new();
                if let Some(file) = selected {
                    detail_lines.extend(render_diff_detail_view_lines(
                        file,
                        source.data.hunks_for_path(&file.path),
                        72,
                    ));
                } else {
                    detail_lines.push("No file selected".to_string());
                }
                BetterViewPanel::new(format!(
                    "Uncommitted changes / {}",
                    selected.map(|file| file.path.as_str()).unwrap_or("detail")
                ))
                .summary(format!("source={} {}", source.label, stats))
                .sections_title("Diff")
                .sections(
                    selected
                        .map(|file| vec![file.path.clone()])
                        .unwrap_or_else(|| vec!["Diff".to_string()]),
                    0,
                )
                .detail_title("Diff")
                .detail_lines(if detail_lines.is_empty() {
                    vec![selected_row("No diff", "", true)]
                } else {
                    detail_lines
                })
                .footer("b back | Up/Down scroll | Esc close")
                .render()
            }
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
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

    pub(crate) fn switch_source(&mut self, direction: isize) {
        self.source_index = cycle_index(self.source_index, self.sources.len(), direction);
        self.selected_index = 0;
    }

    pub(crate) fn move_file(&mut self, direction: isize) {
        self.selected_index =
            cycle_index(self.selected_index, self.current_file_count(), direction);
    }

    pub(crate) fn current_file_count(&self) -> usize {
        self.sources
            .get(self.source_index)
            .map(|source| source.data.files.len())
            .unwrap_or(0)
    }
}

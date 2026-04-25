use std::path::{Path, PathBuf};

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::commands;

use super::theme::Theme;

const MAX_ROWS: usize = 6;
const DETAIL_ROWS: u16 = 4;
const ARG_HELP_HEIGHT: u16 = 4;

#[derive(Debug, Clone)]
pub struct CommandPalette {
    active: bool,
    query: String,
    selected: usize,
    filtered: Vec<CommandItem>,
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

#[derive(Debug, Clone)]
struct EditTarget {
    label: String,
    path: PathBuf,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            selected: 0,
            filtered: Vec::new(),
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
    }

    pub fn close(&mut self) {
        self.active = false;
        self.query.clear();
        self.selected = 0;
        self.filtered.clear();
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

    pub fn argument_hint(input: &str, cwd: &Path) -> Option<String> {
        command_from_argument_input(input, cwd).map(|item| item.usage)
    }

    pub fn argument_help_height(input: &str, cwd: &Path) -> u16 {
        if command_from_argument_input(input, cwd).is_some() {
            ARG_HELP_HEIGHT
        } else {
            0
        }
    }

    pub fn preferred_height(&self) -> u16 {
        if !self.active || self.filtered.is_empty() {
            return 0;
        }
        let list_rows = self.filtered.len().min(MAX_ROWS) as u16;
        (list_rows + DETAIL_ROWS + 2).min(12)
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        if area.height == 0 || area.width == 0 || !self.active {
            return;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Commands ")
            .border_style(theme.dim);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.filtered.is_empty() {
            let line = Line::from(Span::styled("No matching commands", theme.dim));
            buf.set_line(inner.x, inner.y, &line, inner.width);
            return;
        }

        let mut lines = Vec::new();
        for (idx, item) in self.filtered.iter().take(MAX_ROWS).enumerate() {
            let selected = idx == self.selected;
            let style = if selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                theme.unselected
            };
            let aliases = if item.aliases.is_empty() {
                String::new()
            } else {
                format!(" ({})", item.aliases.join(", "))
            };
            lines.push(Line::from(vec![
                Span::styled(format!("/{:<18}", item.name), style),
                Span::styled(aliases, theme.dim),
                Span::styled("  ", theme.dim),
                Span::styled(item.description.clone(), style),
            ]));
        }

        if let Some(selected) = self.filtered.get(self.selected) {
            lines.push(Line::default());
            lines.push(Line::from(vec![
                Span::styled("Usage: ", theme.dim),
                Span::styled(selected.usage.clone(), theme.info),
            ]));
            if let Some(example) = selected.examples.first() {
                lines.push(Line::from(vec![
                    Span::styled("Example: ", theme.dim),
                    Span::styled(example.clone(), theme.unselected),
                ]));
            }
            if !selected.edit_targets.is_empty() {
                let links = selected
                    .edit_targets
                    .iter()
                    .map(|target| format!("{}: {}", target.label, file_uri(&target.path)))
                    .collect::<Vec<_>>()
                    .join(" | ");
                lines.push(Line::from(vec![
                    Span::styled("Edit: ", theme.dim),
                    Span::styled(truncate(&links, inner.width as usize), theme.info),
                ]));
            } else {
                lines.push(Line::from(Span::styled(
                    "Enter selects the command; type arguments after the inserted space.",
                    theme.dim,
                )));
            }
        }

        Paragraph::new(lines).render(inner, buf);
    }

    pub fn render_argument_help(
        input: &str,
        cwd: &Path,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        let Some(item) = command_from_argument_input(input, cwd) else {
            return;
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" /{} arguments ", item.name))
            .border_style(theme.dim);
        let inner = block.inner(area);
        block.render(area, buf);

        let rows = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

        let usage = Line::from(vec![
            Span::styled("Usage: ", theme.dim),
            Span::styled(truncate(&item.usage, inner.width as usize), theme.info),
        ]);
        buf.set_line(rows[0].x, rows[0].y, &usage, rows[0].width);

        let example = item
            .examples
            .first()
            .map(|example| format!("Example: {example}"))
            .unwrap_or_else(|| "Type arguments after the command, then press Enter.".to_string());
        let example_line = Line::from(Span::styled(
            truncate(&example, inner.width as usize),
            theme.unselected,
        ));
        buf.set_line(rows[1].x, rows[1].y, &example_line, rows[1].width);

        if !item.edit_targets.is_empty() {
            let links = item
                .edit_targets
                .iter()
                .map(|target| format!("{}: {}", target.label, file_uri(&target.path)))
                .collect::<Vec<_>>()
                .join(" | ");
            let edit_line = Line::from(vec![
                Span::styled("Edit: ", theme.dim),
                Span::styled(truncate(&links, inner.width as usize), theme.link),
            ]);
            buf.set_line(rows[2].x, rows[2].y, &edit_line, rows[2].width);
        }
    }
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

fn filtered_commands(query: &str, cwd: &Path) -> Vec<CommandItem> {
    let mut items: Vec<CommandItem> = commands::get_all_commands()
        .into_iter()
        .filter(|cmd| {
            fuzzy_match(&cmd.name, query)
                || fuzzy_match(&cmd.description, query)
                || cmd.aliases.iter().any(|alias| fuzzy_match(alias, query))
        })
        .map(|cmd| {
            let meta = command_meta(&cmd.name, cwd);
            CommandItem {
                usage: meta.usage,
                examples: meta.examples,
                edit_targets: meta.edit_targets,
                name: cmd.name,
                aliases: cmd.aliases,
                description: cmd.description,
            }
        })
        .collect();
    items.sort_by(|a, b| score(&a.name, query).cmp(&score(&b.name, query)));
    items
}

fn command_from_argument_input(input: &str, cwd: &Path) -> Option<CommandItem> {
    let without_slash = input.strip_prefix('/')?;
    if !without_slash.contains(char::is_whitespace) {
        return None;
    }

    let name = without_slash.split_whitespace().next()?;
    commands::get_all_commands()
        .into_iter()
        .find(|cmd| cmd.name == name || cmd.aliases.iter().any(|alias| alias == name))
        .map(|cmd| {
            let meta = command_meta(&cmd.name, cwd);
            CommandItem {
                usage: meta.usage,
                examples: meta.examples,
                edit_targets: meta.edit_targets,
                name: cmd.name,
                aliases: cmd.aliases,
                description: cmd.description,
            }
        })
}

fn fuzzy_match(candidate: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let mut chars = candidate.chars();
    for q in query.chars() {
        if !chars.any(|c| c.eq_ignore_ascii_case(&q)) {
            return false;
        }
    }
    true
}

fn score(candidate: &str, query: &str) -> usize {
    if query.is_empty() {
        return 0;
    }
    if candidate == query {
        0
    } else if candidate.starts_with(query) {
        1
    } else if candidate.contains(query) {
        2
    } else {
        3
    }
}

struct CommandMeta {
    usage: String,
    examples: Vec<String>,
    edit_targets: Vec<EditTarget>,
}

fn command_meta(name: &str, cwd: &Path) -> CommandMeta {
    match name {
        "mcp" => CommandMeta {
            usage: "/mcp <list|status|add|edit|remove|connect> [args]".to_string(),
            examples: vec![
                "/mcp add ctx7 --command=npx --arg=-y --arg=@upstash/context7-mcp".to_string(),
            ],
            edit_targets: vec![
                EditTarget {
                    label: "user".to_string(),
                    path: cc_config::settings::user_settings_path(),
                },
                EditTarget {
                    label: "project".to_string(),
                    path: cc_config::settings::project_settings_path(cwd),
                },
                EditTarget {
                    label: "local".to_string(),
                    path: cc_config::settings::local_settings_path(cwd),
                },
            ],
        },
        "plugin" => CommandMeta {
            usage: "/plugin <list|installed|disabled|errors|status|enable|disable|uninstall> [id]"
                .to_string(),
            examples: vec!["/plugin enable <plugin-id>".to_string()],
            edit_targets: vec![
                EditTarget {
                    label: "installed".to_string(),
                    path: cc_config::paths::plugins_dir().join("installed_plugins.json"),
                },
                EditTarget {
                    label: "cache".to_string(),
                    path: cc_config::paths::plugins_dir().join("cache"),
                },
            ],
        },
        "skills" => CommandMeta {
            usage: "/skills [name]".to_string(),
            examples: vec!["/skills".to_string()],
            edit_targets: vec![
                EditTarget {
                    label: "user".to_string(),
                    path: cc_config::paths::skills_dir_global(),
                },
                EditTarget {
                    label: "project".to_string(),
                    path: cwd.join(".cc-rust").join("skills"),
                },
            ],
        },
        "keybindings" => CommandMeta {
            usage: "/keybindings [open|status|list|reload|path]".to_string(),
            examples: vec!["/keybindings".to_string()],
            edit_targets: vec![EditTarget {
                label: "user".to_string(),
                path: cc_config::paths::keybindings_path(),
            }],
        },
        "config" => CommandMeta {
            usage: "/config [get|set|unset|list] [key] [value]".to_string(),
            examples: vec!["/config set model claude-sonnet-4".to_string()],
            edit_targets: vec![
                EditTarget {
                    label: "user".to_string(),
                    path: cc_config::settings::user_settings_path(),
                },
                EditTarget {
                    label: "project".to_string(),
                    path: cc_config::settings::project_settings_path(cwd),
                },
            ],
        },
        "memory" => CommandMeta {
            usage: "/memory [show|edit|add] [text]".to_string(),
            examples: vec!["/memory".to_string()],
            edit_targets: vec![
                EditTarget {
                    label: "project".to_string(),
                    path: cwd.join("CLAUDE.md"),
                },
                EditTarget {
                    label: "global".to_string(),
                    path: cc_config::paths::memory_dir_global(),
                },
            ],
        },
        other => CommandMeta {
            usage: format!("/{other} [args]"),
            examples: Vec::new(),
            edit_targets: Vec::new(),
        },
    }
}

fn truncate(s: &str, max_width: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_width {
        s.to_string()
    } else if max_width <= 3 {
        ".".repeat(max_width)
    } else {
        format!("{}...", chars[..max_width - 3].iter().collect::<String>())
    }
}

fn file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    let encoded = normalized.replace(' ', "%20");

    if encoded.starts_with('/') {
        format!("file://{encoded}")
    } else {
        format!("file:///{encoded}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_opens_filtered_palette() {
        let mut palette = CommandPalette::new();
        palette.sync_from_input("/mc", Path::new("/repo"));
        assert!(palette.active());
        assert_eq!(palette.filtered[0].name, "mcp");
    }

    #[test]
    fn whitespace_closes_palette_for_argument_entry() {
        let mut palette = CommandPalette::new();
        palette.sync_from_input("/mcp ", Path::new("/repo"));
        assert!(!palette.active());
    }

    #[test]
    fn selected_command_keeps_space_for_arguments() {
        let mut palette = CommandPalette::new();
        palette.sync_from_input("/mcp", Path::new("/repo"));
        assert_eq!(palette.selected_command_input().as_deref(), Some("/mcp "));
    }

    #[test]
    fn complex_commands_show_edit_targets() {
        let mut palette = CommandPalette::new();
        palette.sync_from_input("/plugin", Path::new("/repo"));
        let item = palette
            .filtered
            .iter()
            .find(|item| item.name == "plugin")
            .expect("plugin command");
        assert!(item
            .edit_targets
            .iter()
            .any(|target| target.path.ends_with("installed_plugins.json")));
    }

    #[test]
    fn argument_hint_is_shown_after_command_selection() {
        let hint = CommandPalette::argument_hint("/mcp ", Path::new("/repo"));
        assert_eq!(
            hint.as_deref(),
            Some("/mcp <list|status|add|edit|remove|connect> [args]")
        );
        assert_eq!(
            CommandPalette::argument_help_height("/mcp ", Path::new("/repo")),
            ARG_HELP_HEIGHT
        );
    }

    #[test]
    fn edit_targets_render_as_file_links() {
        let uri = file_uri(Path::new("C:\\Users\\me\\settings.json"));
        assert_eq!(uri, "file:///C:/Users/me/settings.json");
    }
}

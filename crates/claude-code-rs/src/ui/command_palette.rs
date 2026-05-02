use std::path::{Path, PathBuf};

use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};

use crate::commands;

use super::theme::Theme;

const MAX_ROWS: usize = 6;
const DETAIL_ROWS: u16 = 4;
const ARG_HELP_BASE_HEIGHT: u16 = 5;
const MAX_EDIT_ROWS: usize = 2;

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
    display: String,
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

    pub fn argument_hint(input: &str, cwd: &Path) -> Option<String> {
        command_from_argument_input(input, cwd).map(|item| item.usage)
    }

    pub fn argument_help_height(input: &str, cwd: &Path) -> u16 {
        command_from_argument_input(input, cwd)
            .map(|item| ARG_HELP_BASE_HEIGHT + argument_edit_row_count(&item) as u16)
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

        let selected_item = self.filtered.get(self.selected);
        let detail_rows = selected_item
            .map(|item| palette_detail_rows(item, inner.width as usize))
            .unwrap_or(DETAIL_ROWS);
        let visible_rows = (inner.height as usize)
            .saturating_sub(detail_rows as usize)
            .min(MAX_ROWS);
        let mut lines = Vec::new();
        let visible_start = visible_window_start(self.filtered.len(), self.selected, visible_rows);
        for (idx, item) in self
            .filtered
            .iter()
            .enumerate()
            .skip(visible_start)
            .take(visible_rows)
        {
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

        if let Some(selected) = selected_item {
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
                for (idx, text) in
                    edit_target_lines(&selected.edit_targets, inner.width as usize, MAX_EDIT_ROWS)
                        .into_iter()
                        .enumerate()
                {
                    let label = if idx == 0 { "Edit: " } else { "      " };
                    lines.push(Line::from(vec![
                        Span::styled(label, theme.dim),
                        Span::styled(text, theme.info),
                    ]));
                }
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

        let usage = Line::from(vec![
            Span::styled("Usage: ", theme.dim),
            Span::styled(truncate(&item.usage, inner.width as usize), theme.info),
        ]);
        set_inner_line(buf, inner, 0, &usage);

        let example = item
            .examples
            .first()
            .map(|example| format!("Example: {example}"))
            .unwrap_or_else(|| "Type arguments after the command, then press Enter.".to_string());
        let example_line = Line::from(Span::styled(
            truncate(&example, inner.width as usize),
            theme.unselected,
        ));
        set_inner_line(buf, inner, 1, &example_line);

        let about_line = Line::from(vec![
            Span::styled("About: ", theme.dim),
            Span::styled(
                truncate(&item.description, inner.width as usize),
                theme.unselected,
            ),
        ]);
        set_inner_line(buf, inner, 2, &about_line);

        if !item.edit_targets.is_empty() {
            for (idx, text) in
                edit_target_lines(&item.edit_targets, inner.width as usize, MAX_EDIT_ROWS)
                    .into_iter()
                    .enumerate()
            {
                let row_idx = 3 + idx;
                if row_idx as u16 >= inner.height {
                    break;
                }
                let label = if idx == 0 { "Edit: " } else { "      " };
                let edit_line = Line::from(vec![
                    Span::styled(label, theme.dim),
                    Span::styled(text, theme.link),
                ]);
                set_inner_line(buf, inner, row_idx as u16, &edit_line);
            }
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

fn visible_window_start(total: usize, selected: usize, max_rows: usize) -> usize {
    if max_rows == 0 || total <= max_rows {
        return 0;
    }

    selected.saturating_add(1).saturating_sub(max_rows)
}

fn set_inner_line(buf: &mut Buffer, inner: Rect, row: u16, line: &Line<'_>) {
    if row < inner.height {
        buf.set_line(inner.x, inner.y + row, line, inner.width);
    }
}

fn argument_edit_row_count(item: &CommandItem) -> usize {
    if item.edit_targets.is_empty() {
        0
    } else {
        item.edit_targets.len().min(MAX_EDIT_ROWS)
    }
}

fn palette_detail_rows(item: &CommandItem, width: usize) -> u16 {
    let edit_rows = if item.edit_targets.is_empty() {
        1
    } else {
        edit_target_lines(&item.edit_targets, width, MAX_EDIT_ROWS)
            .len()
            .max(1)
    };
    3 + edit_rows as u16
}

struct CommandMeta {
    usage: String,
    examples: Vec<String>,
    edit_targets: Vec<EditTarget>,
}

impl EditTarget {
    fn new(label: &str, path: PathBuf, cwd: &Path) -> Self {
        let display = display_path(&path, cwd);
        Self {
            label: label.to_string(),
            display,
        }
    }
}

fn simple_meta(usage: &str, examples: &[&str]) -> CommandMeta {
    CommandMeta {
        usage: usage.to_string(),
        examples: examples
            .iter()
            .map(|example| (*example).to_string())
            .collect(),
        edit_targets: Vec::new(),
    }
}

fn command_meta(name: &str, cwd: &Path) -> CommandMeta {
    match name {
        "add-dir" => simple_meta("/add-dir <path>", &["/add-dir ../shared"]),
        "advisor" => simple_meta(
            "/advisor <model|clear|off>",
            &["/advisor sonnet", "/advisor clear"],
        ),
        "agents" => simple_meta(
            "/agents <list|show> [name]",
            &["/agents", "/agents show executor"],
        ),
        "assistant" => simple_meta("/assistant", &["/assistant"]),
        "audit-export" => simple_meta("/audit-export", &["/audit-export"]),
        "branch" => simple_meta(
            "/branch [title]",
            &["/branch", "/branch try alternate approach"],
        ),
        "brief" => simple_meta("/brief [on|off|status]", &["/brief on"]),
        "btw" => simple_meta("/btw <question>", &["/btw explain the last error"]),
        "channels" => simple_meta("/channels [list|status]", &["/channels status"]),
        "chrome" => simple_meta("/chrome [status|reconnect|help]", &["/chrome reconnect"]),
        "clear" => simple_meta("/clear", &["/clear"]),
        "commit" => simple_meta("/commit [message]", &["/commit fix command palette layout"]),
        "compact" => simple_meta("/compact [instructions]", &["/compact focus on UI fixes"]),
        "context" => simple_meta("/context [json|raw|tui|full]", &["/context json"]),
        "copy" => simple_meta("/copy", &["/copy"]),
        "cost" => simple_meta("/cost", &["/cost"]),
        "daemon" => simple_meta("/daemon [status|stop]", &["/daemon status"]),
        "diff" => simple_meta("/diff", &["/diff"]),
        "doctor" => simple_meta("/doctor [all|auth|settings|mcp|terminal]", &["/doctor"]),
        "dream" => simple_meta("/dream [--days N]", &["/dream --days 7"]),
        "effort" => simple_meta("/effort <low|medium|high>", &["/effort medium"]),
        "exit" => simple_meta("/exit", &["/exit"]),
        "export" => simple_meta("/export [path]", &["/export session.md"]),
        "extra-usage" => simple_meta("/extra-usage", &["/extra-usage"]),
        "fast" => simple_meta("/fast [on|off|status]", &["/fast on"]),
        "files" => simple_meta("/files", &["/files"]),
        "gbranch" => simple_meta("/gbranch [branch-name]", &["/gbranch feature/ui-fix"]),
        "help" => simple_meta("/help [command]", &["/help mcp"]),
        "hooks" => simple_meta(
            "/hooks <list|path|open> [event|layer]",
            &["/hooks list PreToolUse", "/hooks open project"],
        ),
        "ide" => simple_meta(
            "/ide [detect|status|select|clear|reconnect]",
            &["/ide status"],
        ),
        "init" => simple_meta("/init", &["/init"]),
        "insights" => simple_meta("/insights [fast|full]", &["/insights"]),
        "login" => simple_meta(
            "/login [anthropic|codex|api-key]",
            &["/login", "/login codex"],
        ),
        "login-code" => simple_meta("/login-code <authorization-code>", &["/login-code abc123"]),
        "logout" => simple_meta("/logout", &["/logout"]),
        "loop" => simple_meta(
            "/loop <interval|list|remove|trigger|pause|resume> ...",
            &["/loop 5m /status"],
        ),
        "mcp" => CommandMeta {
            usage: "/mcp <list|status|add|edit|remove|connect|disconnect|reconnect>".to_string(),
            examples: vec![
                "/mcp add ctx7 --command=npx --arg=-y --arg=@upstash/context7-mcp".to_string(),
            ],
            edit_targets: vec![
                EditTarget::new("user", cc_config::settings::user_settings_path(), cwd),
                EditTarget::new("project", cc_config::settings::project_settings_path(cwd), cwd),
                EditTarget::new("local", cc_config::settings::local_settings_path(cwd), cwd),
            ],
        },
        "model" => simple_meta("/model [model-id|alias]", &["/model sonnet"]),
        "model-add" => simple_meta(
            "/model-add <name> [input_price output_price]",
            &["/model-add gpt-4o 2.50 10.00"],
        ),
        "notify" => simple_meta("/notify [status|test|on|off]", &["/notify status"]),
        "permissions" => simple_meta(
            "/permissions [mode|allow|ask|deny|session-grant|clear-session-grants|reset] ...",
            &["/permissions", "/permissions mode plan"],
        ),
        "plan" => simple_meta(
            "/plan <enter|show|status|approve|reject|link|classify> ...",
            &["/plan enter refactor command palette"],
        ),
        "plugin" => CommandMeta {
            usage: "/plugin <list|installed|disabled|errors|status|enable|disable|uninstall> [id]"
                .to_string(),
            examples: vec!["/plugin enable <plugin-id>".to_string()],
            edit_targets: vec![
                EditTarget::new(
                    "installed",
                    cc_config::paths::plugins_dir().join("installed_plugins.json"),
                    cwd,
                ),
                EditTarget::new("cache", cc_config::paths::plugins_dir().join("cache"), cwd),
            ],
        },
        "rate-limit-options" => simple_meta("/rate-limit-options", &["/rate-limit-options"]),
        "recap" => simple_meta("/recap [short|long]", &["/recap short"]),
        "reload-plugins" => simple_meta("/reload-plugins", &["/reload-plugins"]),
        "rename" => simple_meta("/rename [title|clear]", &["/rename UI command fixes"]),
        "resume" => simple_meta("/resume <session-id|recent>", &["/resume recent"]),
        "review" => simple_meta("/review <pr-number|url|branch>", &["/review 123"]),
        "rewind" => simple_meta("/rewind <turn|message-id>", &["/rewind 2"]),
        "sandbox" => simple_meta(
            "/sandbox <status|mode|network|paths> ...",
            &["/sandbox mode workspace", "/sandbox network off"],
        ),
        "schedule" => simple_meta(
            "/schedule <add|list|pause|trigger|remove> ...",
            &["/schedule add 30m /status"],
        ),
        "security-review" => simple_meta(
            "/security-review [scope]",
            &["/security-review crates/claude-code-rs/src/ui"],
        ),
        "session" => simple_meta("/session [list|list all]", &["/session list"]),
        "session-export" => simple_meta(
            "/session-export <list|session-id|path>",
            &["/session-export list"],
        ),
        "simplify" => simple_meta(
            "/simplify [--single|-1] [scope]",
            &["/simplify --single crates/claude-code-rs/src/ui"],
        ),
        "skills" => CommandMeta {
            usage: "/skills [name]".to_string(),
            examples: vec!["/skills".to_string()],
            edit_targets: vec![
                EditTarget::new("user", cc_config::paths::skills_dir_global(), cwd),
                EditTarget::new("project", cwd.join(".cc-rust").join("skills"), cwd),
            ],
        },
        "sleep" => simple_meta("/sleep <seconds>", &["/sleep 60"]),
        "status" => simple_meta("/status", &["/status"]),
        "statusline" => simple_meta(
            "/statusline <status|set|clear|enable|disable|test|payload|refresh|timeout|padding> ...",
            &["/statusline show"],
        ),
        "tasks" => simple_meta(
            "/tasks <list|show|stop|delete> [id]",
            &["/tasks", "/tasks show task-123"],
        ),
        "team" => simple_meta(
            "/team <status|list|create|spawn|send|kill|leave|delete> ...",
            &["/team create ui-fix", "/team status"],
        ),
        "team-onboarding" => simple_meta("/team-onboarding [team-name]", &["/team-onboarding"]),
        "terminal-setup" => simple_meta(
            "/terminal-setup [env|tips|all]",
            &["/terminal-setup tips"],
        ),
        "version" => simple_meta("/version", &["/version"]),
        "voice" => simple_meta(
            "/voice [status|on|off|toggle|diagnose]",
            &["/voice status"],
        ),
        "keybindings" => CommandMeta {
            usage: "/keybindings [open|status|list|reload|path]".to_string(),
            examples: vec!["/keybindings".to_string()],
            edit_targets: vec![EditTarget::new(
                "user",
                cc_config::paths::keybindings_path(),
                cwd,
            )],
        },
        "config" => CommandMeta {
            usage: "/config <show|sources|schema|set|reset> [key] [value]".to_string(),
            examples: vec!["/config set model claude-sonnet-4".to_string()],
            edit_targets: vec![
                EditTarget::new("user", cc_config::settings::user_settings_path(), cwd),
                EditTarget::new("project", cc_config::settings::project_settings_path(cwd), cwd),
            ],
        },
        "memory" => CommandMeta {
            usage: "/memory <show|edit|add> [text]".to_string(),
            examples: vec!["/memory".to_string()],
            edit_targets: vec![
                EditTarget::new("project", cwd.join("CLAUDE.md"), cwd),
                EditTarget::new("global", cc_config::paths::memory_dir_global(), cwd),
            ],
        },
        other => CommandMeta {
            usage: format!("/{other}"),
            examples: vec![format!("/{other}")],
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

fn edit_target_lines(targets: &[EditTarget], max_width: usize, max_lines: usize) -> Vec<String> {
    if targets.is_empty() || max_width == 0 || max_lines == 0 {
        return Vec::new();
    }

    let fragments = targets
        .iter()
        .map(|target| format!("{}: {}", target.label, target.display))
        .collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut current = String::new();

    for fragment in fragments {
        let prefix_width = if lines.is_empty() {
            "Edit: ".len()
        } else {
            "      ".len()
        };
        let available = max_width.saturating_sub(prefix_width).max(1);
        let separator = if current.is_empty() { "" } else { " | " };
        let candidate_width = current.len() + separator.len() + fragment.len();

        if candidate_width <= available {
            if !current.is_empty() {
                current.push_str(separator);
            }
            current.push_str(&fragment);
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            if lines.len() == max_lines {
                return lines;
            }
        }

        let available = max_width.saturating_sub("      ".len()).max(1);
        current = truncate(&fragment, available);
    }

    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    }

    lines
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

fn display_path(path: &Path, cwd: &Path) -> String {
    if !cwd.as_os_str().is_empty() && cwd != Path::new(".") {
        if let Ok(relative) = path.strip_prefix(cwd) {
            return format!("./{}", normalize_path(relative));
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        if let Ok(relative) = path.strip_prefix(&current_dir) {
            return format!("./{}", normalize_path(relative));
        }
    }

    let data_root = cc_config::paths::data_root();
    if let Ok(relative) = path.strip_prefix(&data_root) {
        let prefix = if std::env::var_os("CC_RUST_HOME").is_some() {
            "$CC_RUST_HOME"
        } else {
            "~/.cc-rust"
        };
        return format!("{}/{}", prefix, normalize_path(relative));
    }

    file_uri(path)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

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
            .any(|target| target.display.ends_with("installed_plugins.json")));
    }

    #[test]
    fn argument_hint_is_shown_after_command_selection() {
        let hint = CommandPalette::argument_hint("/mcp ", Path::new("/repo"));
        assert_eq!(
            hint.as_deref(),
            Some("/mcp <list|status|add|edit|remove|connect|disconnect|reconnect>")
        );
        assert_eq!(
            CommandPalette::argument_help_height("/mcp ", Path::new("/repo")),
            ARG_HELP_BASE_HEIGHT + MAX_EDIT_ROWS as u16
        );
    }

    #[test]
    fn argument_help_uses_three_content_rows() {
        let area = Rect::new(
            0,
            0,
            100,
            CommandPalette::argument_help_height("/status ", Path::new("/repo")),
        );
        let mut buf = Buffer::empty(area);

        CommandPalette::render_argument_help(
            "/status ",
            Path::new("/repo"),
            area,
            &mut buf,
            &Theme::default(),
        );

        let rendered = buffer_text(&buf, area);
        assert!(rendered.contains("Usage: /status"));
        assert!(rendered.contains("Example: /status"));
        assert!(rendered.contains("About: Show session status"));
    }

    #[test]
    fn argument_help_wraps_edit_targets_without_dropping_file_names() {
        let cwd = snapshot_cwd();
        let area = Rect::new(
            0,
            0,
            110,
            CommandPalette::argument_help_height("/mcp ", &cwd),
        );
        let mut buf = Buffer::empty(area);

        CommandPalette::render_argument_help("/mcp ", &cwd, area, &mut buf, &Theme::default());

        let rendered = buffer_text(&buf, area);
        assert!(rendered.contains("settings.local.json"));
    }

    #[test]
    fn command_meta_fallback_does_not_show_generic_args_placeholder() {
        let meta = command_meta("future-command", Path::new("/repo"));
        assert_eq!(meta.usage, "/future-command");
        assert!(!meta.usage.contains("[args]"));
        assert_eq!(meta.examples, vec!["/future-command".to_string()]);
    }

    #[test]
    fn visible_window_tracks_selection_beyond_first_page() {
        assert_eq!(visible_window_start(12, 0, MAX_ROWS), 0);
        assert_eq!(visible_window_start(12, MAX_ROWS - 1, MAX_ROWS), 0);
        assert_eq!(visible_window_start(12, MAX_ROWS, MAX_ROWS), 1);
        assert_eq!(visible_window_start(12, 11, MAX_ROWS), 6);
    }

    #[test]
    fn render_scrolls_command_list_to_selected_item() {
        let mut palette = CommandPalette::new();
        palette.sync_from_input("/", Path::new("/repo"));
        for _ in 0..MAX_ROWS {
            palette.handle_key(KeyCode::Down);
        }

        let selected_name = palette.filtered[palette.selected].name.clone();
        let first_name = palette.filtered[0].name.clone();
        let area = Rect::new(0, 0, 100, palette.preferred_height());
        let mut buf = Buffer::empty(area);
        palette.render(area, &mut buf, &Theme::default());

        let rendered = buffer_text(&buf, area);
        assert!(rendered.contains(&format!("/{selected_name}")));
        assert!(!rendered.contains(&format!("/{first_name}  ")));
    }

    #[test]
    fn edit_targets_render_as_file_links() {
        let uri = file_uri(Path::new("C:\\Users\\me\\settings.json"));
        assert_eq!(uri, "file:///C:/Users/me/settings.json");
    }

    #[test]
    fn display_path_prefers_readable_project_relative_paths() {
        let cwd = Path::new("C:\\repo");
        let path = cwd.join(".cc-rust").join("settings.json");
        assert_eq!(display_path(&path, cwd), "./.cc-rust/settings.json");
    }

    #[test]
    fn snapshot_command_palette_root_and_filtered_views() {
        let cwd = snapshot_cwd();

        insta::assert_snapshot!(
            "command_palette_root_100x12",
            render_palette_snapshot("/", &cwd, 100, 12, &[])
        );
        insta::assert_snapshot!(
            "command_palette_after_page_down_100x12",
            render_palette_snapshot("/", &cwd, 100, 12, &[KeyCode::PageDown])
        );
        insta::assert_snapshot!(
            "command_palette_mcp_filtered_100x12",
            render_palette_snapshot("/mcp", &cwd, 100, 12, &[])
        );
        insta::assert_snapshot!(
            "command_palette_plugin_filtered_100x12",
            render_palette_snapshot("/plugin", &cwd, 100, 12, &[])
        );
    }

    #[test]
    fn snapshot_all_command_argument_help_views() {
        let cwd = snapshot_cwd();
        let mut rendered = String::new();

        for command in commands::get_all_commands() {
            let input = format!("/{} ", command.name);
            let area = Rect::new(
                0,
                0,
                110,
                CommandPalette::argument_help_height(&input, &cwd),
            );
            let mut buf = Buffer::empty(area);

            CommandPalette::render_argument_help(&input, &cwd, area, &mut buf, &Theme::default());

            rendered.push_str(&format!("## /{}\n", command.name));
            rendered.push_str(&buffer_text_trimmed(&buf, area));
            rendered.push('\n');
        }

        insta::assert_snapshot!(
            "command_argument_help_all_commands_110w",
            normalize_snapshot_text(rendered)
        );
    }

    fn render_palette_snapshot(
        input: &str,
        cwd: &Path,
        width: u16,
        height: u16,
        keys: &[KeyCode],
    ) -> String {
        let mut palette = CommandPalette::new();
        palette.sync_from_input(input, cwd);
        for key in keys {
            palette.handle_key(*key);
        }

        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| {
                palette.render(frame.area(), frame.buffer_mut(), &Theme::default());
            })
            .expect("draw");

        normalize_snapshot_text(terminal.backend().to_string())
    }

    fn snapshot_cwd() -> PathBuf {
        PathBuf::from("C:\\cc-rust-snapshot")
    }

    fn normalize_snapshot_text(mut text: String) -> String {
        for raw_home in [std::env::var_os("USERPROFILE"), std::env::var_os("HOME")]
            .into_iter()
            .flatten()
        {
            let home = PathBuf::from(raw_home).to_string_lossy().replace('\\', "/");
            text = text.replace(&home, "<HOME>");
            text = text.replace(&home.replace(' ', "%20"), "<HOME>");
        }

        text.replace("C:/cc-rust-snapshot", "<WORKSPACE>")
    }

    fn buffer_text_trimmed(buf: &Buffer, area: Rect) -> String {
        let mut lines = Vec::new();
        for y in area.y..area.y + area.height {
            let mut line = String::new();
            for x in area.x..area.x + area.width {
                line.push_str(buf[(x, y)].symbol());
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }

    fn buffer_text(buf: &Buffer, area: Rect) -> String {
        let mut text = String::new();
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                text.push_str(buf[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }
}

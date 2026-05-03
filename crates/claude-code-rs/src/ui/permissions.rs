// BEGIN generated upstream permissions modules
// Rust-side permission modules mirrored from upstream React components.
#[allow(dead_code)]
pub mod ask_user_question_permission_request;
#[allow(dead_code)]
pub mod bash_permission_request;
#[allow(dead_code)]
pub mod computer_use_approval;
#[allow(dead_code)]
pub mod enter_plan_mode_permission_request;
#[allow(dead_code)]
pub mod exit_plan_mode_permission_request;
#[allow(dead_code)]
pub mod fallback_permission_request;
#[allow(dead_code)]
pub mod file_edit_permission_request;
#[allow(dead_code)]
pub mod file_permission_dialog;
#[allow(dead_code)]
pub mod file_write_permission_request;
#[allow(dead_code)]
pub mod filesystem_permission_request;
#[allow(dead_code)]
pub mod hooks;
#[allow(dead_code)]
pub mod monitor_permission_request;
#[allow(dead_code)]
pub mod notebook_edit_permission_request;
#[allow(dead_code)]
pub mod permission_decision_debug_info;
#[allow(dead_code)]
pub mod permission_dialog;
#[allow(dead_code)]
pub mod permission_explanation;
#[allow(dead_code)]
pub mod permission_prompt;
#[allow(dead_code)]
pub mod permission_request;
#[allow(dead_code)]
pub mod permission_request_title;
#[allow(dead_code)]
pub mod permission_rule_explanation;
#[allow(dead_code)]
pub mod power_shell_permission_request;
#[allow(dead_code)]
pub mod review_artifact_permission_request;
#[allow(dead_code)]
pub mod rules;
#[allow(dead_code)]
pub mod sandbox_permission_request;
#[allow(dead_code)]
pub mod sed_edit_permission_request;
#[allow(dead_code)]
pub mod shell_permission_helpers;
#[allow(dead_code)]
pub mod skill_permission_request;
#[allow(dead_code)]
pub mod use_shell_permission_feedback;
#[allow(dead_code)]
pub mod utils;
#[allow(dead_code)]
pub mod web_fetch_permission_request;
#[allow(dead_code)]
pub mod worker_badge;
#[allow(dead_code)]
pub mod worker_pending_permission;
// END generated upstream permissions modules

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
#[allow(unused_imports)]
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::approval_overlay::ApprovalKind;
use super::theme::Theme;

/// The user's response to a permission prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionChoice {
    /// Allow this single invocation.
    Allow,
    /// Deny this single invocation.
    Deny,
    /// Always allow this tool (add a permanent rule).
    AlwaysAllow,
}

const DEFAULT_OPTIONS: [&str; 3] = ["Allow", "Deny", "Always Allow"];

/// An overlay dialog that asks the user whether to permit a tool invocation.
pub struct PermissionDialog {
    /// Name of the tool requesting permission.
    pub tool_name: String,
    /// Abbreviated / formatted tool input.
    pub tool_input: String,
    /// Human-readable description of what the tool wants to do.
    pub message: String,
    /// Current permission category inferred from the tool name.
    kind: ApprovalKind,
    /// Button labels shown at the bottom of the dialog.
    options: Vec<String>,
    /// Currently highlighted choice.
    pub selected: usize,
}

impl PermissionDialog {
    pub fn new(tool_name: &str, input: &str, message: &str) -> Self {
        let kind = approval_kind(tool_name, input, message);
        Self {
            tool_name: tool_name.to_string(),
            tool_input: input.to_string(),
            message: message.to_string(),
            kind,
            options: default_options(),
            selected: 0,
        }
    }

    /// Handle a key event. Returns `Some(choice)` when the user confirms a
    /// selection with Enter, or makes a direct choice via a keyboard shortcut.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<PermissionChoice> {
        let choice_count = self.options.len().max(DEFAULT_OPTIONS.len());

        match (key.modifiers, key.code) {
            // Navigation
            (_, KeyCode::Left) | (_, KeyCode::Char('h')) => {
                self.selected = self.selected.saturating_sub(1);
            }
            (_, KeyCode::Right) | (_, KeyCode::Char('l')) => {
                if self.selected + 1 < choice_count {
                    self.selected += 1;
                }
            }
            (_, KeyCode::Tab) => {
                self.selected = (self.selected + 1) % choice_count;
            }
            (KeyModifiers::SHIFT, KeyCode::BackTab) => {
                self.selected = if self.selected == 0 {
                    choice_count - 1
                } else {
                    self.selected - 1
                };
            }

            // Confirm
            (_, KeyCode::Enter) => {
                return Some(self.choice_for_selected());
            }

            // Quick keys
            (_, KeyCode::Char('y')) | (_, KeyCode::Char('Y')) => {
                return Some(PermissionChoice::Allow);
            }
            (_, KeyCode::Char('n')) | (_, KeyCode::Char('N')) => {
                return Some(PermissionChoice::Deny);
            }
            (_, KeyCode::Char('a')) | (_, KeyCode::Char('A')) => {
                return Some(PermissionChoice::AlwaysAllow);
            }
            (_, KeyCode::Esc) => {
                return Some(PermissionChoice::Deny);
            }

            _ => {}
        }
        None
    }

    /// Render the permission dialog as a centered overlay.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let dialog_width = (area.width * 68 / 100).max(48).min(area.width);
        let dialog_height = 13u16.min(area.height).max(8);
        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;
        let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

        Clear.render_ref(dialog_area, buf);

        let block = Block::default()
            .title(format!(" Permission Required - {} ", self.kind.title()))
            .borders(Borders::ALL)
            .border_style(theme.warning)
            .style(Style::default());
        let inner = block.inner(dialog_area);
        block.render_ref(dialog_area, buf);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        let chunks = Layout::vertical([
            Constraint::Length(2), // tool info
            Constraint::Min(2),    // request body
            Constraint::Length(2), // buttons + hint
        ])
        .split(inner);

        let tool_info = vec![
            Line::from(vec![
                Span::styled("Tool: ", theme.dim),
                Span::styled(self.tool_name.clone(), theme.tool_name),
            ]),
            Line::from(vec![
                Span::styled("Kind: ", theme.dim),
                Span::styled(
                    self.kind.title().trim_end_matches('?').to_string(),
                    theme.info,
                ),
            ]),
        ];
        Paragraph::new(tool_info).render_ref(chunks[0], buf);

        let request_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(theme.warning);
        let request_inner = request_block.inner(chunks[1]);
        request_block.render_ref(chunks[1], buf);
        if request_inner.height > 0 && request_inner.width > 0 {
            let body_label = body_label_for_kind(&self.kind);
            let body_text = self.body_text();
            let mut body_lines = vec![Line::from(vec![
                Span::styled(format!("{body_label}: "), theme.dim),
                Span::styled(
                    truncate_str(&body_text, request_inner.width as usize),
                    theme.warning,
                ),
            ])];

            if !self.message.trim().is_empty() && self.message.trim() != body_text.trim() {
                body_lines.push(Line::from(vec![Span::styled(
                    truncate_str(&self.message, request_inner.width as usize),
                    theme.dim,
                )]));
            }

            Paragraph::new(body_lines)
                .wrap(Wrap { trim: true })
                .render_ref(request_inner, buf);
        }

        let labels = self.normalized_options();
        let button_spans: Vec<Span> = labels
            .iter()
            .enumerate()
            .flat_map(|(idx, label)| {
                let style = if idx == self.selected {
                    theme.selected
                } else {
                    theme.unselected
                };
                let shortcut = shortcut_for_label(label);
                let mut spans = vec![Span::styled(format!(" {} ", label), style)];
                if let Some(shortcut) = shortcut {
                    spans.push(Span::styled(format!("({shortcut})"), theme.dim));
                }
                if idx + 1 < labels.len() {
                    spans.push(Span::raw("  "));
                }
                spans
            })
            .collect();

        let button_line = Line::from(button_spans);
        let button_y = chunks[2].y + (chunks[2].height.saturating_sub(2)) / 2;
        buf.set_line(chunks[2].x + 1, button_y, &button_line, chunks[2].width);

        let hint = Line::from(vec![Span::styled(
            "Arrow keys or hotkeys. Enter confirms. Esc denies.",
            theme.dim,
        )]);
        buf.set_line(
            chunks[2].x + 1,
            chunks[2].y + chunks[2].height.saturating_sub(1),
            &hint,
            chunks[2].width,
        );
    }

    fn body_text(&self) -> String {
        if !self.tool_input.trim().is_empty() {
            self.tool_input.clone()
        } else if !self.message.trim().is_empty() {
            self.message.clone()
        } else {
            "(no details supplied)".to_string()
        }
    }

    fn normalized_options(&self) -> Vec<String> {
        if self.options.is_empty() {
            default_options()
        } else {
            self.options.clone()
        }
    }

    fn choice_for_selected(&self) -> PermissionChoice {
        let labels = self.normalized_options();
        labels
            .get(self.selected.min(labels.len().saturating_sub(1)))
            .map(|label| choice_for_label(label).unwrap_or(PermissionChoice::Allow))
            .unwrap_or(PermissionChoice::Allow)
    }
}

fn approval_kind(tool_name: &str, input: &str, message: &str) -> ApprovalKind {
    let joined = format!("{tool_name} {input} {message}").to_ascii_lowercase();
    let subject = if !input.trim().is_empty() {
        input.trim().to_string()
    } else if !message.trim().is_empty() {
        message.trim().to_string()
    } else {
        tool_name.to_string()
    };

    if joined.contains("bash")
        || joined.contains("shell")
        || joined.contains("terminal")
        || joined.contains("powershell")
    {
        ApprovalKind::Bash { command: subject }
    } else if joined.contains("write") || joined.contains("edit") || joined.contains("patch") {
        ApprovalKind::FileEdit { path: subject }
    } else if joined.contains("fetch") || joined.contains("web") || joined.contains("url") {
        ApprovalKind::WebFetch { url: subject }
    } else if joined.contains("mcp") {
        ApprovalKind::Mcp {
            server: tool_name.to_string(),
            action: subject,
        }
    } else if joined.contains("ask") || joined.contains("question") {
        ApprovalKind::UserInput { prompt: subject }
    } else {
        ApprovalKind::Fallback { summary: subject }
    }
}

fn body_label_for_kind(kind: &ApprovalKind) -> &'static str {
    match kind {
        ApprovalKind::Bash { .. } => "Command",
        ApprovalKind::FileEdit { .. } => "File",
        ApprovalKind::WebFetch { .. } => "URL",
        ApprovalKind::Mcp { .. } => "Action",
        ApprovalKind::UserInput { .. } => "Prompt",
        ApprovalKind::Fallback { .. } => "Details",
    }
}

fn default_options() -> Vec<String> {
    DEFAULT_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn choice_for_label(label: &str) -> Option<PermissionChoice> {
    let lower = label.to_ascii_lowercase();
    if lower.contains("always") {
        Some(PermissionChoice::AlwaysAllow)
    } else if lower.contains("allow") && !lower.contains("always") {
        Some(PermissionChoice::Allow)
    } else if lower.contains("deny") || lower.contains("reject") || lower == "no" {
        Some(PermissionChoice::Deny)
    } else {
        None
    }
}

fn shortcut_for_label(label: &str) -> Option<&'static str> {
    match choice_for_label(label) {
        Some(PermissionChoice::Allow) => Some("y"),
        Some(PermissionChoice::Deny) => Some("n"),
        Some(PermissionChoice::AlwaysAllow) => Some("a"),
        None => None,
    }
}

// ── Helper rendering traits ───────────────────────────────────────────────

trait RenderRef {
    fn render_ref(&self, area: Rect, buf: &mut Buffer);
}

impl RenderRef for Clear {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.reset();
                }
            }
        }
    }
}

impl RenderRef for Block<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        ratatui::widgets::Widget::render(self.clone(), area, buf);
    }
}

impl RenderRef for Paragraph<'_> {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        ratatui::widgets::Widget::render(self.clone(), area, buf);
    }
}

/// Truncate a string to at most `max_chars` characters, appending "..." if
/// truncation occurred.
fn truncate_str(s: &str, max_chars: usize) -> String {
    if max_chars < 4 {
        return s.chars().take(max_chars).collect();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let truncated: String = chars[..max_chars - 3].iter().collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::ask_user_question_permission_request::ask_user_question_permission_request::render_ask_user_question_permission_request;
    use super::ask_user_question_permission_request::preview_box::render_preview_box;
    use super::ask_user_question_permission_request::preview_question_view::render_preview_question_view;
    use super::ask_user_question_permission_request::question_navigation_bar::render_question_navigation_bar;
    use super::ask_user_question_permission_request::question_view::render_question_view;
    use super::ask_user_question_permission_request::submit_questions_view::render_submit_questions_view;
    use super::ask_user_question_permission_request::use_multiple_choice_state::{
        render_multiple_choice_state, MultipleChoiceState,
    };
    use super::bash_permission_request::bash_permission_request::render_bash_permission_request;
    use super::bash_permission_request::bash_tool_use_options::render_bash_tool_use_options;
    use super::computer_use_approval::computer_use_approval::render_computer_use_approval;
    use super::enter_plan_mode_permission_request::enter_plan_mode_permission_request::render_enter_plan_mode_permission_request;
    use super::exit_plan_mode_permission_request::exit_plan_mode_permission_request::render_exit_plan_mode_permission_request;
    use super::fallback_permission_request::render_fallback_permission_request;
    use super::file_edit_permission_request::file_edit_permission_request::render_file_edit_permission_request;
    use super::file_permission_dialog::file_permission_dialog::render_file_permission_dialog;
    use super::file_permission_dialog::ide_diff_config::{render_ide_diff_config, IdeDiffConfig};
    use super::file_permission_dialog::permission_options::file_permission_options;
    use super::file_permission_dialog::use_file_permission_dialog::FilePermissionDialogState;
    use super::file_permission_dialog::use_permission_handler::{
        describe_file_permission_decision, FilePermissionDecision,
    };
    use super::file_write_permission_request::file_write_permission_request::render_file_write_permission_request;
    use super::file_write_permission_request::file_write_tool_diff::render_file_write_tool_diff;
    use super::filesystem_permission_request::filesystem_permission_request::render_filesystem_permission_request;
    use super::hooks::{render_permission_hook_event, PermissionHookEvent};
    use super::monitor_permission_request::monitor_permission_request::render_monitor_permission_request;
    use super::notebook_edit_permission_request::notebook_edit_permission_request::render_notebook_edit_permission_request;
    use super::notebook_edit_permission_request::notebook_edit_tool_diff::render_notebook_edit_tool_diff;
    use super::permission_decision_debug_info::render_permission_decision_debug_info;
    use super::permission_dialog::render_permission_dialog_summary;
    use super::permission_explanation::render_permission_explanation;
    use super::permission_prompt::{render_permission_prompt, PermissionPromptState};
    use super::permission_request::{basic_permission_request, render_permission_request_surface};
    use super::permission_request_title::render_permission_request_title;
    use super::permission_rule_explanation::render_permission_rule_explanation;
    use super::power_shell_permission_request::power_shell_permission_request::render_power_shell_permission_request;
    use super::power_shell_permission_request::powershell_tool_use_options::render_powershell_tool_use_options;
    use super::review_artifact_permission_request::review_artifact_permission_request::render_review_artifact_permission_request;
    use super::rules::add_permission_rules::render_add_permission_rules;
    use super::rules::add_workspace_directory::render_add_workspace_directory;
    use super::rules::permission_rule_description::render_permission_rule_description;
    use super::rules::permission_rule_input::{
        render_permission_rule_input, validate_rule_pattern, PermissionRuleInputState,
    };
    use super::rules::permission_rule_list::render_permission_rule_list;
    use super::rules::recent_denials_tab::render_recent_denials_tab;
    use super::rules::remove_workspace_directory::render_remove_workspace_directory;
    use super::rules::workspace_tab::render_workspace_tab;
    use super::rules::{PermissionRule, RecentDenial, WorkspaceDirectory};
    use super::sandbox_permission_request::render_sandbox_permission_request;
    use super::sed_edit_permission_request::sed_edit_permission_request::render_sed_edit_permission_request;
    use super::shell_permission_helpers::{
        render_shell_permission_details, shell_permission_options, ShellKind,
    };
    use super::skill_permission_request::skill_permission_request::render_skill_permission_request;
    use super::use_shell_permission_feedback::{
        render_shell_permission_feedback, ShellPermissionFeedback,
    };
    use super::utils::{
        command_preview, default_permission_options, normalize_multiline, path_action_summary,
        render_bullets, render_key_values, render_permission_options, render_permission_request,
        shell_risk_hint, truncate_middle, PermissionDecision, PermissionRequestView,
        PermissionScope,
    };
    use super::web_fetch_permission_request::web_fetch_permission_request::render_web_fetch_permission_request;
    use super::worker_badge::render_worker_badge;
    use super::worker_pending_permission::render_worker_pending_permission;

    #[test]
    fn snapshot_permission_component_helpers() {
        let mut choices = MultipleChoiceState::new(vec![
            "Inspect logs".to_string(),
            "Ask operator".to_string(),
            "Abort".to_string(),
        ]);
        choices.select_next();
        choices.toggle_selected();

        let rules = vec![
            PermissionRule::new(
                "Bash(ls*)",
                PermissionDecision::Allow,
                PermissionScope::Project,
                ".cc-rust/settings.json",
            ),
            PermissionRule::new(
                "Write(src/**)",
                PermissionDecision::Deny,
                PermissionScope::Local,
                "policy",
            ),
        ];
        let directories = vec![
            WorkspaceDirectory {
                path: "F:/repo".to_string(),
                trusted: true,
            },
            WorkspaceDirectory {
                path: "F:/repo/tmp".to_string(),
                trusted: false,
            },
        ];
        let denials = vec![RecentDenial {
            tool_name: "bash".to_string(),
            pattern: "rm -rf".to_string(),
            reason: "destructive command".to_string(),
        }];
        let rule_input =
            PermissionRuleInputState::new("Bash(cargo test*)", PermissionDecision::Allow);
        let prompt =
            PermissionPromptState::new("Allow this action?", default_permission_options(), 2);
        let base_request = PermissionRequestView::new("Permission required", "bash", "cargo test")
            .with_detail("cwd: F:/repo")
            .with_risk("shell command")
            .for_worker("executor");
        let mut file_dialog = FilePermissionDialogState::new("src/lib.rs");
        file_dialog.select_next(3);

        let rendered = [
            section("utils-request", render_permission_request(&base_request)),
            section(
                "utils-options",
                render_permission_options(&default_permission_options(), 1).join("\n"),
            ),
            section(
                "utils-key-values",
                render_key_values("Rows", &[("tool", "bash"), ("decision", "allow")]),
            ),
            section(
                "utils-bullets",
                render_bullets("Notes", &["first", "second"]),
            ),
            section("utils-truncate", truncate_middle("abcdefghijklmnop", 10)),
            section("utils-normalize", normalize_multiline("a\n\nb").join("|")),
            section(
                "utils-command-preview",
                command_preview("cargo test\ncargo fmt"),
            ),
            section("utils-risk", shell_risk_hint("rm -rf target").to_string()),
            section(
                "utils-path-action",
                path_action_summary("edit", "src/main.rs"),
            ),
            section(
                "permission-request",
                render_permission_request_surface(&basic_permission_request("read", "src/lib.rs")),
            ),
            section("permission-prompt", render_permission_prompt(&prompt)),
            section(
                "permission-title",
                render_permission_request_title("bash", "cargo test", true),
            ),
            section(
                "permission-explanation",
                render_permission_explanation("writes project files", &["updates lockfile"]),
            ),
            section(
                "permission-rule-explanation",
                render_permission_rule_explanation(
                    "Bash(cargo*)",
                    PermissionDecision::AlwaysAllow,
                    PermissionScope::Project,
                ),
            ),
            section(
                "permission-debug",
                render_permission_decision_debug_info(
                    "bash",
                    "Bash(cargo*)",
                    "project",
                    Some("allow Bash(cargo*)"),
                ),
            ),
            section(
                "permission-dialog",
                render_permission_dialog_summary(&base_request, 80),
            ),
            section(
                "fallback",
                render_fallback_permission_request(
                    "custom_tool",
                    "run custom action",
                    &["opaque input"],
                ),
            ),
            section(
                "sandbox",
                render_sandbox_permission_request("read-only", "write target", &["target denied"]),
            ),
            section(
                "hooks",
                render_permission_hook_event(&PermissionHookEvent {
                    hook_name: "PreToolUse".to_string(),
                    matcher: "Bash(*)".to_string(),
                    decision: "allow".to_string(),
                    notes: vec!["matched project hook".to_string()],
                }),
            ),
            section(
                "shell-details",
                render_shell_permission_details(ShellKind::Bash, "cargo test"),
            ),
            section(
                "shell-options",
                render_permission_options(&shell_permission_options("curl https://example.com"), 0)
                    .join("\n"),
            ),
            section(
                "shell-feedback",
                render_shell_permission_feedback(&ShellPermissionFeedback {
                    command: "cargo test".to_string(),
                    accepted: true,
                    rule_saved: false,
                    message: "running now".to_string(),
                }),
            ),
            section("worker-badge", render_worker_badge("executor", 2)),
            section(
                "worker-pending",
                render_worker_pending_permission("executor", "write", "src/lib.rs", 1),
            ),
            section("ask-choice-state", render_multiple_choice_state(&choices)),
            section(
                "ask-preview-box",
                render_preview_box("Question", "Pick one"),
            ),
            section(
                "ask-preview-question",
                render_preview_question_view("Which path?", &["src", "tests"]),
            ),
            section(
                "ask-navigation",
                render_question_navigation_bar(2, 3, true, true),
            ),
            section(
                "ask-question-view",
                render_question_view("Choose", &choices),
            ),
            section("ask-submit", render_submit_questions_view(2, 3)),
            section(
                "ask-request",
                render_ask_user_question_permission_request("Need input", &choices, 2, 3),
            ),
            section(
                "bash-request",
                render_bash_permission_request("cargo test", 0),
            ),
            section(
                "bash-options",
                render_bash_tool_use_options("cargo test", 2),
            ),
            section(
                "powershell-request",
                render_power_shell_permission_request("Get-ChildItem", 1),
            ),
            section(
                "powershell-options",
                render_powershell_tool_use_options("Get-ChildItem", 0),
            ),
            section(
                "ide-diff",
                render_ide_diff_config(&IdeDiffConfig {
                    enabled: true,
                    editor: "code".to_string(),
                    supports_inline_diff: true,
                }),
            ),
            section(
                "file-options",
                render_permission_options(&file_permission_options("src/lib.rs", true), 2)
                    .join("\n"),
            ),
            section(
                "file-handler",
                describe_file_permission_decision(&FilePermissionDecision {
                    decision: PermissionDecision::Allow,
                    path: "src/lib.rs".to_string(),
                    save_rule: true,
                }),
            ),
            section(
                "file-dialog",
                render_file_permission_dialog(&file_dialog, "edit", "+3 -1"),
            ),
            section(
                "file-write-diff",
                render_file_write_tool_diff("src/lib.rs", 3, 1),
            ),
            section(
                "file-write-request",
                render_file_write_permission_request("src/lib.rs", 3, 1, 1),
            ),
            section(
                "file-edit-request",
                render_file_edit_permission_request("src/lib.rs", "replace range", 0),
            ),
            section(
                "filesystem-request",
                render_filesystem_permission_request("read", "src", 2),
            ),
            section(
                "computer-use",
                render_computer_use_approval("click", "browser window", true),
            ),
            section(
                "enter-plan",
                render_enter_plan_mode_permission_request("Need design first"),
            ),
            section(
                "exit-plan",
                render_exit_plan_mode_permission_request("implement helpers", &["cargo test"]),
            ),
            section(
                "monitor",
                render_monitor_permission_request("cargo test", 60, 1),
            ),
            section(
                "review-artifact",
                render_review_artifact_permission_request("docs/report.md", "reviewer"),
            ),
            section(
                "sed-edit",
                render_sed_edit_permission_request("src/lib.rs", "foo", "bar", 1),
            ),
            section(
                "notebook-diff",
                render_notebook_edit_tool_diff("analysis.ipynb", 2, "python"),
            ),
            section(
                "notebook-request",
                render_notebook_edit_permission_request("analysis.ipynb", 2, "python", 0),
            ),
            section(
                "skill-request",
                render_skill_permission_request("lint-fix", "run skill", 1),
            ),
            section(
                "web-fetch",
                render_web_fetch_permission_request("https://example.com", "GET", 2),
            ),
            section(
                "rule-description",
                render_permission_rule_description(&rules[0]),
            ),
            section("rule-input", render_permission_rule_input(&rule_input)),
            section(
                "rule-input-validation",
                validate_rule_pattern("bad\npattern").unwrap_err(),
            ),
            section("rule-list", render_permission_rule_list(&rules, 1)),
            section(
                "add-rules",
                render_add_permission_rules(std::slice::from_ref(&rule_input)),
            ),
            section(
                "add-workspace",
                render_add_workspace_directory("F:/repo", true),
            ),
            section(
                "remove-workspace",
                render_remove_workspace_directory("F:/repo/tmp", 2),
            ),
            section("workspace-tab", render_workspace_tab(&directories, 0)),
            section("recent-denials", render_recent_denials_tab(&denials)),
        ]
        .join("\n\n");

        insta::assert_snapshot!("permission_component_helpers", rendered);
    }

    fn section(name: &str, body: String) -> String {
        format!("## {name}\n{body}")
    }
}

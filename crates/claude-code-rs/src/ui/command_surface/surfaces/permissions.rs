use crossterm::event::KeyEvent;

use crate::ui::better_view_panel::{selected_row, BetterViewPanel};
use crate::ui::command_surface::CommandSurfaceOutcome;
use crate::ui::form_navigation::{FormOption, FormTab, TabbedFormEvent, TabbedFormState};
use crate::ui::permissions::rules::permission_rule_list::render_permission_rule_list;
use crate::ui::permissions::rules::recent_denials_tab::render_recent_denials_tab;
use crate::ui::permissions::rules::workspace_tab::render_workspace_tab;
use crate::ui::permissions::rules::{PermissionRule, RecentDenial, WorkspaceDirectory};
use crate::ui::permissions::utils::{PermissionDecision, PermissionScope};
use cc_engine::types::app_state::AppState;
use cc_engine::types::tool::{PermissionMode, ToolPermissionContext, ToolPermissionRulesBySource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionsSurface {
    pub(crate) state: TabbedFormState,
    rules: Vec<PermissionRule>,
    workspace_directories: Vec<WorkspaceDirectory>,
    recent_denials: Vec<RecentDenial>,
}

impl PermissionsSurface {
    pub(crate) fn new(app_state: &AppState) -> Self {
        let perm = &app_state.tool_permission_context;
        let mut rules = Vec::new();
        collect_rules(
            &mut rules,
            &perm.always_deny_rules,
            PermissionDecision::Deny,
        );
        collect_rules(&mut rules, &perm.always_ask_rules, PermissionDecision::Ask);
        collect_rules(
            &mut rules,
            &perm.always_allow_rules,
            PermissionDecision::AlwaysAllow,
        );
        collect_rules(
            &mut rules,
            &perm.session_allow_rules,
            PermissionDecision::Allow,
        );
        let mut workspace_directories = perm
            .additional_working_directories
            .values()
            .map(|directory| WorkspaceDirectory {
                path: directory.path.clone(),
                trusted: !directory.read_only,
            })
            .collect::<Vec<_>>();
        workspace_directories.sort_by(|a, b| a.path.cmp(&b.path));

        Self {
            state: TabbedFormState::new(
                "Permissions",
                vec![
                    status_tab(perm),
                    rules_tab(&rules),
                    modes_tab(perm),
                    workspace_directories_tab(&workspace_directories),
                    recent_denials_tab(&[]),
                    mutate_tab(),
                ],
            ),
            rules,
            workspace_directories,
            recent_denials: Vec::new(),
        }
    }

    pub(crate) fn render(&self) -> String {
        let sections = self
            .state
            .tabs
            .iter()
            .map(|tab| tab.label.clone())
            .collect::<Vec<_>>();
        let mut detail_lines = if let Some(tab) = self.state.active_tab() {
            tab.options
                .iter()
                .enumerate()
                .map(|(idx, option)| {
                    selected_row(
                        &option.label,
                        &option.description,
                        idx == self.state.selected_index,
                    )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if self.active_tab_id() == Some("rules") {
            detail_lines.push(String::new());
            detail_lines.extend(
                render_permission_rule_list(&self.rules, self.state.selected_index)
                    .lines()
                    .map(str::to_string),
            );
        } else if self.active_tab_id() == Some("workspace") {
            detail_lines.push(String::new());
            detail_lines.extend(
                render_workspace_tab(&self.workspace_directories, self.state.selected_index)
                    .lines()
                    .map(str::to_string),
            );
        } else if self.active_tab_id() == Some("denials") {
            detail_lines.push(String::new());
            detail_lines.extend(
                render_recent_denials_tab(&self.recent_denials)
                    .lines()
                    .map(str::to_string),
            );
        }
        BetterViewPanel::new("Permissions")
            .summary(format!(
                "mode={} rules={}",
                self.state
                    .active_tab()
                    .map(|tab| tab.label.as_str())
                    .unwrap_or("unknown"),
                self.rules.len()
            ))
            .sections(sections, self.state.active_tab)
            .detail_title(
                self.state
                    .active_tab()
                    .map(|tab| tab.label.clone())
                    .unwrap_or_else(|| "Detail".to_string()),
            )
            .detail_lines(detail_lines)
            .footer("Left/Right section | Up/Down navigate | Enter select | Esc close")
            .render()
    }

    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        match self.state.handle_key(key) {
            TabbedFormEvent::Selected { option_id, .. } => match option_id.as_str() {
                "show" => CommandSurfaceOutcome::Submit("/permissions".to_string()),
                "mode-default" => {
                    CommandSurfaceOutcome::Submit("/permissions mode default".to_string())
                }
                "mode-auto" => {
                    CommandSurfaceOutcome::Submit("/permissions mode auto --confirm".to_string())
                }
                "mode-bypass" => {
                    CommandSurfaceOutcome::Submit("/permissions mode bypass --confirm".to_string())
                }
                "mode-plan" => CommandSurfaceOutcome::Submit("/permissions mode plan".to_string()),
                "mode-accept-edits" => {
                    CommandSurfaceOutcome::Submit("/permissions mode acceptEdits".to_string())
                }
                "mode-dont-ask" => {
                    CommandSurfaceOutcome::Submit("/permissions mode dontAsk".to_string())
                }
                "add-allow" => CommandSurfaceOutcome::FillPrompt(
                    "/permissions allow <rule> --project".to_string(),
                ),
                "add-ask" => CommandSurfaceOutcome::FillPrompt(
                    "/permissions ask <rule> --project".to_string(),
                ),
                "add-deny" => CommandSurfaceOutcome::FillPrompt(
                    "/permissions deny <rule> --project".to_string(),
                ),
                "session-grant" => {
                    CommandSurfaceOutcome::FillPrompt("/permissions session-grant ".to_string())
                }
                "clear-session" => {
                    CommandSurfaceOutcome::Submit("/permissions clear-session-grants".to_string())
                }
                "reset" => CommandSurfaceOutcome::Submit("/permissions reset".to_string()),
                _ => CommandSurfaceOutcome::None,
            },
            _ => CommandSurfaceOutcome::None,
        }
    }

    fn active_tab_id(&self) -> Option<&str> {
        self.state.active_tab().map(|tab| tab.id.as_str())
    }
}

fn status_tab(perm: &ToolPermissionContext) -> FormTab {
    FormTab::new(
        "status",
        "Status",
        vec![
            FormOption::new("show", "Show effective permissions").with_description(format!(
                "mode={}; auto={}; bypass={}",
                perm.mode.as_str(),
                availability_label(perm.is_auto_mode_available.unwrap_or(true)),
                availability_label(perm.is_bypass_permissions_mode_available),
            )),
        ],
    )
}

fn rules_tab(rules: &[PermissionRule]) -> FormTab {
    FormTab::new(
        "rules",
        "Rules",
        vec![FormOption::new("show", "Review effective rules")
            .with_description(format!("{} rule(s) from settings/session", rules.len()))],
    )
}

fn workspace_directories_tab(directories: &[WorkspaceDirectory]) -> FormTab {
    FormTab::new(
        "workspace",
        "Workspace",
        vec![
            FormOption::new("workspace-show", "Review workspace directories")
                .with_description(format!("{} additional directorie(s)", directories.len())),
        ],
    )
}

fn recent_denials_tab(denials: &[RecentDenial]) -> FormTab {
    FormTab::new(
        "denials",
        "Denials",
        vec![FormOption::new("denials-show", "Review recent denials")
            .with_description(format!("{} recent denial(s)", denials.len()))],
    )
}

fn modes_tab(perm: &ToolPermissionContext) -> FormTab {
    let mut auto = FormOption::new("mode-auto", "Auto mode")
        .with_description("requires explicit opt-in; classifier reviews prompts");
    if perm.is_auto_mode_available == Some(false) {
        auto = auto.disabled();
    }
    let mut bypass = FormOption::new("mode-bypass", "Bypass permissions")
        .with_description("requires explicit danger confirmation");
    if !perm.is_bypass_permissions_mode_available {
        bypass = bypass.disabled();
    }

    FormTab::new(
        "modes",
        "Modes",
        vec![
            mode_option("mode-default", "Default", PermissionMode::Default, perm),
            auto,
            bypass,
            mode_option("mode-plan", "Plan", PermissionMode::Plan, perm),
            mode_option(
                "mode-accept-edits",
                "Accept edits",
                PermissionMode::AcceptEdits,
                perm,
            ),
            mode_option("mode-dont-ask", "Don't ask", PermissionMode::DontAsk, perm),
        ],
    )
}

fn mutate_tab() -> FormTab {
    FormTab::new(
        "mutate",
        "Mutate",
        vec![
            FormOption::new("add-allow", "Add allow rule")
                .with_description("fills /permissions allow <rule> --project"),
            FormOption::new("add-ask", "Add ask rule")
                .with_description("fills /permissions ask <rule> --project"),
            FormOption::new("add-deny", "Add deny rule")
                .with_description("fills /permissions deny <rule> --project"),
            FormOption::new("session-grant", "Add session grant")
                .with_description("transient allow; cleared on session end"),
            FormOption::new("clear-session", "Clear session grants")
                .with_description("drops transient allow rules"),
            FormOption::new("reset", "Reset in-memory rules")
                .with_description("does not edit .cc-rust settings files"),
        ],
    )
}

fn mode_option(
    id: impl Into<String>,
    label: impl Into<String>,
    mode: PermissionMode,
    perm: &ToolPermissionContext,
) -> FormOption {
    let current = if perm.mode == mode {
        "current"
    } else {
        "select"
    };
    FormOption::new(id, label).with_description(current)
}

fn collect_rules(
    out: &mut Vec<PermissionRule>,
    rules_by_source: &ToolPermissionRulesBySource,
    decision: PermissionDecision,
) {
    let mut sorted: Vec<(&String, &Vec<String>)> = rules_by_source.iter().collect();
    sorted.sort_by_key(|(source, _)| (*source).clone());
    for (source, rules) in sorted {
        for rule in rules {
            out.push(PermissionRule::new(
                rule,
                decision,
                scope_for_source(source),
                source,
            ));
        }
    }
}

fn scope_for_source(source: &str) -> PermissionScope {
    match source {
        "session" => PermissionScope::Session,
        "local" => PermissionScope::Local,
        "policy" | "managed" => PermissionScope::Policy,
        _ => PermissionScope::Project,
    }
}

fn availability_label(enabled: bool) -> &'static str {
    if enabled {
        "available"
    } else {
        "disabled"
    }
}

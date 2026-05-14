use crossterm::event::KeyEvent;

use crate::ui::command_surface::CommandSurfaceOutcome;
use crate::ui::form_navigation::{FormOption, FormTab, TabbedFormEvent, TabbedFormState};
use crate::ui::permissions::rules::permission_rule_list::render_permission_rule_list;
use crate::ui::permissions::rules::PermissionRule;
use crate::ui::permissions::utils::{PermissionDecision, PermissionScope};
use cc_engine::types::app_state::AppState;
use cc_engine::types::tool::{PermissionMode, ToolPermissionContext, ToolPermissionRulesBySource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionsSurface {
    pub(crate) state: TabbedFormState,
    rules: Vec<PermissionRule>,
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

        Self {
            state: TabbedFormState::new(
                "Permissions",
                vec![
                    status_tab(perm),
                    rules_tab(&rules),
                    modes_tab(perm),
                    mutate_tab(),
                ],
            ),
            rules,
        }
    }

    pub(crate) fn render(&self) -> String {
        let mut lines = self.state.render_lines();
        if self.active_tab_id() == Some("rules") {
            lines.push(String::new());
            lines.push(render_permission_rule_list(
                &self.rules,
                self.state.selected_index,
            ));
        }
        lines.join("\n")
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

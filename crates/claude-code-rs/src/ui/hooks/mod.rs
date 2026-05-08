//! Rust-side hooks configuration UI surfaces.

#[allow(dead_code)]
pub mod hooks_config_menu;
#[allow(dead_code)]
pub mod prompt_dialog;
#[allow(dead_code)]
pub mod select_event_mode;
#[allow(dead_code)]
pub mod select_hook_mode;
#[allow(dead_code)]
pub mod select_matcher_mode;
#[allow(dead_code)]
pub mod view_hook_mode;

#[cfg(test)]
mod tests {
    use super::hooks_config_menu::{render_hooks_config_menu, HookConfigSummary};
    use super::prompt_dialog::PromptDialogState;
    use super::select_event_mode::{render_select_event_mode, HookEvent};
    use super::select_hook_mode::{render_select_hook_mode, HookCommand};
    use super::select_matcher_mode::{render_select_matcher_mode, HookMatcher};
    use super::view_hook_mode::{render_view_hook_mode, HookView};

    #[test]
    fn snapshot_hooks_surfaces() {
        let commands = vec![
            HookCommand::new("cargo fmt --check"),
            HookCommand {
                command: "cargo test -p claude-code-rs".to_string(),
                enabled: false,
                timeout_seconds: Some(30),
            },
        ];
        let matchers = vec![HookMatcher::all_tools(), HookMatcher::for_tool("Bash")];
        let view = HookView {
            event: HookEvent::PreToolUse,
            matcher: matchers[1].clone(),
            commands: commands.clone(),
        };
        let mut prompt = PromptDialogState::new("Add hook", "Command to run");
        prompt.input = "cargo check".to_string();

        let rendered = [
            section(
                "config",
                render_hooks_config_menu(
                    &[HookConfigSummary {
                        event: HookEvent::PreToolUse,
                        matcher_count: 2,
                        command_count: 2,
                    }],
                    0,
                ),
            ),
            section("event", render_select_event_mode(HookEvent::PostToolUse)),
            section("matcher", render_select_matcher_mode(&matchers, 1)),
            section("hook", render_select_hook_mode(&commands, 1)),
            section("prompt", prompt.render()),
            section("view", render_view_hook_mode(&view)),
        ]
        .join("\n\n");

        insta::assert_snapshot!("hooks_surfaces", rendered);
    }

    fn section(name: &str, body: impl AsRef<str>) -> String {
        format!("## {name}\n{}", body.as_ref())
    }
}

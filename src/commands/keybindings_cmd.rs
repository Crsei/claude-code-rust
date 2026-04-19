//! `/keybindings` slash command — open, create, list, or reload the user
//! keybindings file.
//!
//! Usage:
//!   /keybindings                 show status + file path
//!   /keybindings open            ensure file exists and open in `$EDITOR`
//!   /keybindings list [ctx]      list effective bindings (optionally for a
//!                                context)
//!   /keybindings reload          force a reload now
//!   /keybindings path            print the config file path
//!
//! `~/.cc-rust/keybindings.json` is created from an empty template when
//! missing.

use std::io::Write;

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::config::paths;
use crate::keybindings::config::EMPTY_TEMPLATE;
use crate::keybindings::context::Context as KbContext;
use crate::keybindings::registry::KeybindingRegistry;

pub struct KeybindingsHandler;

#[async_trait]
impl CommandHandler for KeybindingsHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();
        let mut parts = args.split_whitespace();
        let sub = parts.next().unwrap_or("").to_ascii_lowercase();

        // Use the registry held on AppState — shared with any other UI
        // surfaces — rather than constructing a fresh one per invocation.
        let reg = ctx.app_state.keybindings.clone();

        match sub.as_str() {
            "" | "status" | "show" => Ok(CommandResult::Output(render_status(&reg))),
            "path" => Ok(CommandResult::Output(format!(
                "{}",
                paths::keybindings_path().display()
            ))),
            "open" | "edit" => Ok(CommandResult::Output(open_for_edit(&reg))),
            "reload" | "refresh" => Ok(CommandResult::Output(reload_now(&reg))),
            "list" => {
                let filter_ctx = parts.next().and_then(|s| s.parse::<KbContext>().ok());
                Ok(CommandResult::Output(list_bindings(&reg, filter_ctx)))
            }
            other => Ok(CommandResult::Output(format!(
                "Unknown /keybindings subcommand '{}'.\n\nUsage:\n  \
                 /keybindings                 — show status\n  \
                 /keybindings open            — create (if missing) and open in $EDITOR\n  \
                 /keybindings list [context]  — list effective bindings\n  \
                 /keybindings reload          — force a reload\n  \
                 /keybindings path            — print config file path",
                other
            ))),
        }
    }
}

fn render_status(reg: &KeybindingRegistry) -> String {
    let path = reg.user_path().unwrap_or_else(paths::keybindings_path);
    let exists = path.exists();
    reg.refresh_if_changed();
    let issues = reg.last_issues();
    let total = reg.all_bindings().len();

    let mut out = String::new();
    out.push_str("Keybindings\n");
    out.push_str("───────────\n");
    out.push_str(&format!("  Config path: {}\n", path.display()));
    out.push_str(&format!(
        "  File:        {}\n",
        if exists { "exists" } else { "not created yet" }
    ));
    out.push_str(&format!("  Effective bindings: {}\n", total));
    if !issues.is_empty() {
        out.push_str("\nConfig issues:\n");
        for i in &issues {
            out.push_str(&format!("  - {}\n", i));
        }
    }
    out.push_str(
        "\nTip: run `/keybindings open` to create and edit the file, or \
         `/keybindings list` to see every effective binding.\n",
    );
    out
}

fn open_for_edit(reg: &KeybindingRegistry) -> String {
    let path = reg.user_path().unwrap_or_else(paths::keybindings_path);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return format!(
                "Error: could not create parent directory {}: {}",
                parent.display(),
                e
            );
        }
    }
    let mut created = false;
    if !path.exists() {
        match std::fs::File::create(&path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(EMPTY_TEMPLATE.as_bytes()) {
                    return format!("Error: could not write template: {}", e);
                }
                created = true;
            }
            Err(e) => return format!("Error: could not create {}: {}", path.display(), e),
        }
    }

    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .ok();

    match editor {
        Some(ed) if !ed.trim().is_empty() => {
            // Fire-and-forget — we don't block the REPL.
            let status = std::process::Command::new(&ed).arg(&path).status();
            match status {
                Ok(s) if s.success() => format!(
                    "{}Opened {} in {}.",
                    if created { "Created and " } else { "" },
                    path.display(),
                    ed
                ),
                Ok(s) => format!(
                    "{} exited with status {}. File: {}",
                    ed,
                    s.code().unwrap_or(-1),
                    path.display()
                ),
                Err(e) => format!(
                    "Error: could not launch '{}' — {}. File: {}",
                    ed,
                    e,
                    path.display()
                ),
            }
        }
        _ => format!(
            "{}File: {}\n\
             (Set $VISUAL or $EDITOR to auto-open in an editor.)",
            if created {
                "Created keybindings template.\n"
            } else {
                ""
            },
            path.display()
        ),
    }
}

fn reload_now(reg: &KeybindingRegistry) -> String {
    let path = reg.user_path().unwrap_or_else(paths::keybindings_path);
    match reg.reload() {
        Ok(()) => {
            let issues = reg.last_issues();
            if issues.is_empty() {
                format!(
                    "Reloaded {} — {} effective bindings.",
                    path.display(),
                    reg.all_bindings().len()
                )
            } else {
                let mut out = format!(
                    "Reloaded {} with {} issue(s):\n",
                    path.display(),
                    issues.len()
                );
                for i in issues {
                    out.push_str(&format!("  - {}\n", i));
                }
                out
            }
        }
        Err(e) => format!("Error: {}", e),
    }
}

fn list_bindings(reg: &KeybindingRegistry, filter: Option<KbContext>) -> String {
    reg.refresh_if_changed();
    let mut bindings = reg.all_bindings();
    if let Some(ctx) = filter {
        bindings.retain(|(c, _, _)| *c == ctx);
    }
    if bindings.is_empty() {
        return "(no bindings)".to_string();
    }
    let mut out = String::new();
    let mut current_ctx: Option<KbContext> = None;
    for (ctx, chord, action) in bindings {
        if current_ctx != Some(ctx) {
            if current_ctx.is_some() {
                out.push('\n');
            }
            out.push_str(&format!("[{}]\n", ctx.as_str()));
            current_ctx = Some(ctx);
        }
        out.push_str(&format!("  {:<20} → {}\n", chord.display(), action));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::SessionId;
    use crate::types::app_state::AppState;

    fn make_ctx() -> CommandContext {
        CommandContext {
            messages: vec![],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            app_state: AppState::default(),
            session_id: SessionId::new(),
        }
    }

    #[tokio::test]
    async fn status_renders_path_line() {
        let handler = KeybindingsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Config path:"));
                assert!(s.contains("Effective bindings:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn path_subcommand_returns_absolute_path() {
        let handler = KeybindingsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("path", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("keybindings.json"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn list_subcommand_emits_context_headers() {
        let handler = KeybindingsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("list Chat", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("[Chat]"));
                // Chat bindings contain chat:submit
                assert!(s.contains("chat:submit"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_returns_usage() {
        let handler = KeybindingsHandler;
        let mut ctx = make_ctx();
        let result = handler.execute("bogus", &mut ctx).await.unwrap();
        match result {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown /keybindings"));
                assert!(s.contains("Usage:"));
            }
            _ => panic!("expected Output"),
        }
    }
}

//! `/terminal-setup` slash command — diagnose the current terminal
//! environment and print configuration tips (issue #12).
//!
//! Output covers:
//!
//! - detected shell / terminal program / multiplexer
//! - status of the three `CLAUDE_CODE_*` env toggles
//! - tips for Shift+Enter support across common terminals
//! - tmux passthrough advice when `$TMUX` is set
//! - how to turn on OSC 52 clipboard / system-bell notifications
//!
//! The command is read-only — it never mutates the user's config. Writing
//! an actual config template is left to a follow-up issue (see the
//! "建议先做到按当前终端环境输出建议" guidance in the spec).
//!
//! Usage:
//!
//! ```text
//!   /terminal-setup         # print diagnostics + tips
//!   /terminal-setup env     # just the env-var table
//!   /terminal-setup tips    # just the per-terminal tips
//! ```

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::ui::terminal_env::TerminalEnvConfig;

pub struct TerminalSetupHandler;

#[async_trait]
impl CommandHandler for TerminalSetupHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim().to_ascii_lowercase();
        let probe = EnvProbe::from_env();
        let out = match args.as_str() {
            "" | "all" | "full" => render_all(&probe),
            "env" | "config" => render_env(&probe),
            "tips" | "help" => render_tips(&probe),
            other => format!(
                "Unknown /terminal-setup subcommand '{}'.\n\nUsage:\n  \
                 /terminal-setup         — environment diagnostics + tips\n  \
                 /terminal-setup env     — just the env-var table\n  \
                 /terminal-setup tips    — just the per-terminal tips",
                other
            ),
        };
        Ok(CommandResult::Output(out))
    }
}

/// Snapshot of the subset of env vars we care about. Captured all at
/// once so the renderers don't re-query the environment for each field.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvProbe {
    pub term: Option<String>,
    pub term_program: Option<String>,
    pub colorterm: Option<String>,
    pub shell: Option<String>,
    pub tmux: Option<String>,
    pub zellij: Option<String>,
    pub screen_socket: Option<String>,
    pub ssh_tty: Option<String>,
    pub visual: Option<String>,
    pub editor: Option<String>,
    pub vte_version: Option<String>,
    pub wt_session: Option<String>,
    pub term_program_version: Option<String>,
    pub claude_code_no_flicker: Option<String>,
    pub claude_code_disable_mouse: Option<String>,
    pub claude_code_scroll_speed: Option<String>,
}

impl EnvProbe {
    pub fn from_env() -> Self {
        Self::from_iter(std::env::vars())
    }

    pub fn from_iter<I, K, V>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut p = Self::default();
        for (k, v) in iter {
            let v = v.as_ref().to_string();
            let vo = |s: &str| {
                let t = s.trim();
                if t.is_empty() {
                    None
                } else {
                    Some(t.to_string())
                }
            };
            match k.as_ref() {
                "TERM" => p.term = vo(&v),
                "TERM_PROGRAM" => p.term_program = vo(&v),
                "TERM_PROGRAM_VERSION" => p.term_program_version = vo(&v),
                "COLORTERM" => p.colorterm = vo(&v),
                "SHELL" => p.shell = vo(&v),
                "TMUX" => p.tmux = vo(&v),
                "ZELLIJ" => p.zellij = vo(&v),
                "STY" => p.screen_socket = vo(&v),
                "SSH_TTY" => p.ssh_tty = vo(&v),
                "VISUAL" => p.visual = vo(&v),
                "EDITOR" => p.editor = vo(&v),
                "VTE_VERSION" => p.vte_version = vo(&v),
                "WT_SESSION" => p.wt_session = vo(&v),
                "CLAUDE_CODE_NO_FLICKER" => p.claude_code_no_flicker = vo(&v),
                "CLAUDE_CODE_DISABLE_MOUSE" => p.claude_code_disable_mouse = vo(&v),
                "CLAUDE_CODE_SCROLL_SPEED" => p.claude_code_scroll_speed = vo(&v),
                _ => {}
            }
        }
        p
    }

    /// Normalize `$TERM_PROGRAM` into a coarse terminal label so we can
    /// pick the right tip bullet below.
    pub fn terminal_label(&self) -> TerminalLabel {
        if self.wt_session.is_some() {
            return TerminalLabel::WindowsTerminal;
        }
        match self.term_program.as_deref().map(str::to_ascii_lowercase) {
            Some(ref s) if s.contains("iterm") => TerminalLabel::ITerm2,
            Some(ref s) if s.contains("apple_terminal") => TerminalLabel::AppleTerminal,
            Some(ref s) if s.contains("vscode") => TerminalLabel::VsCode,
            Some(ref s) if s.contains("wezterm") => TerminalLabel::WezTerm,
            Some(ref s) if s.contains("alacritty") => TerminalLabel::Alacritty,
            Some(ref s) if s.contains("kitty") => TerminalLabel::Kitty,
            Some(ref s) if s.contains("ghostty") => TerminalLabel::Ghostty,
            Some(ref s) if s.contains("hyper") => TerminalLabel::Hyper,
            Some(ref s) if s.contains("tabby") => TerminalLabel::Tabby,
            _ => {
                if self.vte_version.is_some() {
                    TerminalLabel::GnomeLikeVte
                } else {
                    TerminalLabel::Unknown
                }
            }
        }
    }
}

/// Coarse label for the running terminal — used to pick Shift+Enter tips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLabel {
    ITerm2,
    AppleTerminal,
    VsCode,
    WezTerm,
    Alacritty,
    Kitty,
    Ghostty,
    Hyper,
    Tabby,
    WindowsTerminal,
    GnomeLikeVte,
    Unknown,
}

impl TerminalLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            TerminalLabel::ITerm2 => "iTerm2",
            TerminalLabel::AppleTerminal => "Terminal.app",
            TerminalLabel::VsCode => "VS Code",
            TerminalLabel::WezTerm => "WezTerm",
            TerminalLabel::Alacritty => "Alacritty",
            TerminalLabel::Kitty => "Kitty",
            TerminalLabel::Ghostty => "Ghostty",
            TerminalLabel::Hyper => "Hyper",
            TerminalLabel::Tabby => "Tabby",
            TerminalLabel::WindowsTerminal => "Windows Terminal",
            TerminalLabel::GnomeLikeVte => "GNOME / VTE-based",
            TerminalLabel::Unknown => "unknown",
        }
    }

    /// Short Shift+Enter / multi-line tip for this terminal.
    pub fn shift_enter_tip(self) -> &'static str {
        match self {
            TerminalLabel::ITerm2 => {
                "iTerm2 → Settings → Keys → Key Bindings → map Shift+Return to 'Send Escape Sequence' with value `[27;2;13~`."
            }
            TerminalLabel::AppleTerminal => {
                "Terminal.app → Settings → Profiles → Keyboard → add `Shift+Return` sending `\\033[27;2;13~`."
            }
            TerminalLabel::VsCode => {
                "Add to keybindings.json: `{\"key\":\"shift+enter\",\"command\":\"workbench.action.terminal.sendSequence\",\"args\":{\"text\":\"\\u001b[27;2;13~\"}}`."
            }
            TerminalLabel::WezTerm => {
                "wezterm.lua: `keys = { { key='Enter', mods='SHIFT', action = wezterm.action.SendString('\\x1b[27;2;13~') } }`."
            }
            TerminalLabel::Alacritty => {
                "alacritty.toml: `[[keyboard.bindings]] key=\"Return\" mods=\"Shift\" chars=\"\\x1b[27;2;13~\"`."
            }
            TerminalLabel::Kitty => {
                "kitty.conf: `map shift+enter send_text all \\x1b[27;2;13~`."
            }
            TerminalLabel::Ghostty => {
                "Ghostty honours `keybind = shift+enter=text:\\x1b[27;2;13~` in config.toml."
            }
            TerminalLabel::Hyper => {
                "Hyper: enable `hyper-csi-u` or `hyperterm-shift-enter` plugins for Shift+Enter support."
            }
            TerminalLabel::Tabby => {
                "Tabby → Settings → Hotkeys → add Shift+Enter sending `\\x1b[27;2;13~`."
            }
            TerminalLabel::WindowsTerminal => {
                "Windows Terminal settings.json: add `{ \"command\": { \"action\": \"sendInput\", \"input\": \"\\u001b[27;2;13~\" }, \"keys\": \"shift+enter\" }`."
            }
            TerminalLabel::GnomeLikeVte => {
                "gnome-terminal / tilix: no stable Shift+Enter binding. Try tmux passthrough or use `/export-transcript` for long edits."
            }
            TerminalLabel::Unknown => {
                "If Shift+Enter isn't working, try Alt+Enter, Ctrl+J, or map Shift+Return to emit `\\x1b[27;2;13~`."
            }
        }
    }
}

fn render_all(p: &EnvProbe) -> String {
    let mut out = render_env(p);
    out.push('\n');
    out.push_str(&render_tips(p));
    out
}

fn render_env(p: &EnvProbe) -> String {
    let effective = TerminalEnvConfig::from_iter([
        (
            "CLAUDE_CODE_NO_FLICKER",
            p.claude_code_no_flicker.clone().unwrap_or_default(),
        ),
        (
            "CLAUDE_CODE_DISABLE_MOUSE",
            p.claude_code_disable_mouse.clone().unwrap_or_default(),
        ),
        (
            "CLAUDE_CODE_SCROLL_SPEED",
            p.claude_code_scroll_speed.clone().unwrap_or_default(),
        ),
    ]);

    let mut out = String::new();
    out.push_str("Terminal setup\n");
    out.push_str("──────────────\n");
    out.push_str(&format!("  Detected:   {}\n", p.terminal_label().as_str()));
    out.push_str(&row("TERM", &p.term));
    out.push_str(&row("TERM_PROGRAM", &p.term_program));
    out.push_str(&row("TERM_PROGRAM_VERSION", &p.term_program_version));
    out.push_str(&row("COLORTERM", &p.colorterm));
    out.push_str(&row("SHELL", &p.shell));
    out.push_str(&row("TMUX", &p.tmux));
    out.push_str(&row("ZELLIJ", &p.zellij));
    out.push_str(&row("STY (screen)", &p.screen_socket));
    out.push_str(&row("SSH_TTY", &p.ssh_tty));
    out.push_str(&row("VISUAL", &p.visual));
    out.push_str(&row("EDITOR", &p.editor));

    out.push_str("\nCLAUDE_CODE_* toggles (issue #12)\n");
    out.push_str(&row("CLAUDE_CODE_NO_FLICKER", &p.claude_code_no_flicker));
    out.push_str(&format!(
        "    → synchronized updates: {}\n",
        if effective.sync_updates { "on" } else { "off" }
    ));
    out.push_str(&row(
        "CLAUDE_CODE_DISABLE_MOUSE",
        &p.claude_code_disable_mouse,
    ));
    out.push_str(&format!(
        "    → mouse capture:        {}\n",
        if effective.disable_mouse {
            "disabled (mouse wheel won't be captured)"
        } else {
            "not grabbed (default)"
        }
    ));
    out.push_str(&row(
        "CLAUDE_CODE_SCROLL_SPEED",
        &p.claude_code_scroll_speed,
    ));
    out.push_str(&format!(
        "    → scroll speed:         {} lines / step\n",
        effective.scroll_speed
    ));

    out
}

fn render_tips(p: &EnvProbe) -> String {
    let label = p.terminal_label();
    let mut out = String::new();
    out.push_str("Tips\n");
    out.push_str("────\n");

    out.push_str(&format!(
        "Shift+Enter ({}):\n  • {}\n\n",
        label.as_str(),
        label.shift_enter_tip()
    ));

    if p.tmux.is_some() {
        out.push_str("tmux detected:\n");
        out.push_str(
            "  • Add `set -g extended-keys on` + `set -as terminal-features ',xterm*:extkeys'` for \
             Shift+Enter passthrough.\n",
        );
        out.push_str(
            "  • Use `set -g allow-passthrough on` (tmux ≥ 3.3) so Claude Code can emit \
             notifications / OSC sequences.\n\n",
        );
    }

    out.push_str("Notifications:\n");
    out.push_str(
        "  • Terminal bell / OSC 9 notifications are emitted when the agent finishes a turn. \
         If your terminal suppresses them, check its 'Silence bell' / 'Visual bell' setting.\n",
    );
    out.push_str(
        "  • On Linux desktops, `notify-send` hooks can be wired through `statusLine.command`.\n\n",
    );

    out.push_str("Transcript / focus view (issue #12):\n");
    out.push_str("  • Ctrl+O cycles Prompt → Transcript → Focus.\n");
    out.push_str("  • In transcript mode: `/` search, `n`/`N` next/prev, `e` export to $EDITOR, `q`/Esc exit.\n");

    out
}

fn row(label: &str, value: &Option<String>) -> String {
    let shown = value
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("(unset)");
    format!("  {:<25} {}\n", format!("{}:", label), shown)
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
    async fn default_invocation_prints_diagnostic_table() {
        let handler = TerminalSetupHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("Terminal setup"));
                assert!(s.contains("CLAUDE_CODE_NO_FLICKER"));
                assert!(s.contains("Tips"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn env_subcommand_omits_tips() {
        let handler = TerminalSetupHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("env", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("CLAUDE_CODE_NO_FLICKER"));
                assert!(!s.contains("Tips\n────\n"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn tips_subcommand_omits_env_table() {
        let handler = TerminalSetupHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("tips", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("Tips"));
                assert!(!s.contains("Terminal setup\n──────────────"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[tokio::test]
    async fn unknown_subcommand_returns_usage() {
        let handler = TerminalSetupHandler;
        let mut ctx = make_ctx();
        let r = handler.execute("bogus", &mut ctx).await.unwrap();
        match r {
            CommandResult::Output(s) => {
                assert!(s.contains("Unknown /terminal-setup"));
                assert!(s.contains("Usage:"));
            }
            _ => panic!("expected Output"),
        }
    }

    #[test]
    fn terminal_label_detects_iterm2_even_lowercase() {
        let p = EnvProbe::from_iter(vec![("TERM_PROGRAM", "iTerm.app")]);
        assert_eq!(p.terminal_label(), TerminalLabel::ITerm2);
    }

    #[test]
    fn terminal_label_prefers_windows_terminal_when_wt_session_set() {
        let p = EnvProbe::from_iter(vec![("WT_SESSION", "abc"), ("TERM_PROGRAM", "unknown")]);
        assert_eq!(p.terminal_label(), TerminalLabel::WindowsTerminal);
    }

    #[test]
    fn terminal_label_falls_back_to_vte() {
        let p = EnvProbe::from_iter(vec![("VTE_VERSION", "7206")]);
        assert_eq!(p.terminal_label(), TerminalLabel::GnomeLikeVte);
    }

    #[test]
    fn env_table_shows_effective_scroll_speed() {
        let p = EnvProbe::from_iter(vec![("CLAUDE_CODE_SCROLL_SPEED", "7")]);
        let out = render_env(&p);
        assert!(out.contains("7 lines / step"), "{}", out);
    }
}

//! Terminal environment configuration (issue #12).
//!
//! Mirrors the env-var surface documented in `terminal-configuration.md`:
//!
//! | Variable                    | Effect                                             |
//! |-----------------------------|----------------------------------------------------|
//! | `CLAUDE_CODE_NO_FLICKER`    | `1` forces synchronized-update escape sequences;   |
//! |                             | `0` turns them off. Default is on (we already use  |
//! |                             | them) so this is a way to opt out on terminals     |
//! |                             | that behave badly.                                 |
//! | `CLAUDE_CODE_DISABLE_MOUSE` | Parsed and surfaced in diagnostics, but the current|
//! |                             | Rust TUI does not toggle mouse capture yet. This   |
//! |                             | remains a forward-compatibility flag for future    |
//! |                             | mouse support.                                     |
//! | `CLAUDE_CODE_SCROLL_SPEED`  | Lines per PageUp / PageDown scroll step. Integer,  |
//! |                             | clamped to `[1, 50]`. Default: 5.                  |
//!
//! Boolean parsing follows the conventional `1|true|yes|on` family,
//! case-insensitive; missing / empty values fall through to the default.

use std::env;

/// Resolved terminal environment config. Cheap to clone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalEnvConfig {
    /// Use synchronized-update escape sequences in the render loop. The
    /// cc-rust TUI emits these by default to reduce tearing; this gate is
    /// here so users on broken terminals can turn them off.
    pub sync_updates: bool,
    /// Hint to any future mouse-capture code path: never enable grabs.
    pub disable_mouse: bool,
    /// Lines per scroll step for PageUp / PageDown and related keys.
    pub scroll_speed: u16,
}

impl Default for TerminalEnvConfig {
    fn default() -> Self {
        Self {
            sync_updates: true,
            disable_mouse: false,
            scroll_speed: Self::DEFAULT_SCROLL_SPEED,
        }
    }
}

impl TerminalEnvConfig {
    /// The current TUI does not enable or disable mouse capture based on
    /// `CLAUDE_CODE_DISABLE_MOUSE`; the flag is parsed for diagnostics and
    /// future compatibility only.
    pub const DISABLE_MOUSE_RUNTIME_SUPPORTED: bool = false;
    /// Default scroll speed when no override is set. Exposed publicly
    /// so `/terminal-setup` / tests can surface the same number that
    /// `Default::default()` seeds.
    pub const DEFAULT_SCROLL_SPEED: u16 = 5;
    /// Clamped minimum scroll speed. Below this the UI becomes sluggish.
    pub const MIN_SCROLL_SPEED: u16 = 1;
    /// Clamped maximum scroll speed. Above this `PageUp` effectively
    /// teleports the viewport.
    pub const MAX_SCROLL_SPEED: u16 = 50;

    /// Read all three env vars and build a config, falling back to the
    /// documented defaults on any parse error.
    pub fn from_env() -> Self {
        Self::from_iter(env::vars())
    }

    /// Test-friendly constructor that takes an arbitrary key/value iterator.
    /// Skips keys that aren't recognized.
    pub fn from_iter<I, K, V>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut cfg = Self::default();
        for (k, v) in iter {
            match k.as_ref() {
                "CLAUDE_CODE_NO_FLICKER" => {
                    if let Some(b) = parse_bool(v.as_ref()) {
                        cfg.sync_updates = b;
                    }
                }
                "CLAUDE_CODE_DISABLE_MOUSE" => {
                    if let Some(b) = parse_bool(v.as_ref()) {
                        cfg.disable_mouse = b;
                    }
                }
                "CLAUDE_CODE_SCROLL_SPEED" => {
                    if let Ok(n) = v.as_ref().trim().parse::<u16>() {
                        cfg.scroll_speed = n.clamp(Self::MIN_SCROLL_SPEED, Self::MAX_SCROLL_SPEED);
                    }
                }
                _ => {}
            }
        }
        cfg
    }
}

/// Parsed form of a `$VISUAL` / `$EDITOR` value.
///
/// Transcript export in the current TUI launches the env var as a single
/// executable path, so values that also include arguments are diagnostic-only
/// for now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommand {
    pub raw: String,
    pub program: String,
    pub arguments: Option<String>,
}

impl EditorCommand {
    pub fn has_arguments(&self) -> bool {
        self.arguments
            .as_deref()
            .is_some_and(|args| !args.trim().is_empty())
    }

    pub fn transcript_export_supported(&self) -> bool {
        !self.has_arguments()
    }
}

/// Parse an external-editor env var into `program` plus an optional argument
/// suffix. Quoted executable paths with spaces are treated as a single
/// program.
pub fn parse_editor_command(raw: &str) -> Option<EditorCommand> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (program, trailing) = if let Some(rest) = trimmed.strip_prefix('"') {
        let end = rest.find('"')?;
        (&rest[..end], rest[end + 1..].trim())
    } else if let Some(rest) = trimmed.strip_prefix('\'') {
        let end = rest.find('\'')?;
        (&rest[..end], rest[end + 1..].trim())
    } else {
        match trimmed.find(char::is_whitespace) {
            Some(split_at) => (&trimmed[..split_at], trimmed[split_at..].trim()),
            None => (trimmed, ""),
        }
    };

    let program = program.trim();
    if program.is_empty() {
        return None;
    }

    Some(EditorCommand {
        raw: trimmed.to_string(),
        program: program.to_string(),
        arguments: (!trailing.is_empty()).then(|| trailing.to_string()),
    })
}

/// Permissive boolean parser — accepts the common truthy/falsy spellings.
/// Returns `None` when the value is empty / ambiguous so callers can keep
/// the default.
fn parse_bool(raw: &str) -> Option<bool> {
    let s = raw.trim().to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    match s.as_str() {
        "1" | "true" | "yes" | "on" | "y" | "t" => Some(true),
        "0" | "false" | "no" | "off" | "n" | "f" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        let cfg = TerminalEnvConfig::default();
        assert!(cfg.sync_updates);
        assert!(!cfg.disable_mouse);
        assert_eq!(cfg.scroll_speed, TerminalEnvConfig::DEFAULT_SCROLL_SPEED);
    }

    #[test]
    fn no_flicker_0_disables_sync_updates() {
        let cfg = TerminalEnvConfig::from_iter(vec![("CLAUDE_CODE_NO_FLICKER", "0")]);
        assert!(!cfg.sync_updates);
    }

    #[test]
    fn no_flicker_1_keeps_sync_updates_on() {
        let cfg = TerminalEnvConfig::from_iter(vec![("CLAUDE_CODE_NO_FLICKER", "1")]);
        assert!(cfg.sync_updates);
    }

    #[test]
    fn disable_mouse_accepts_common_truthy() {
        for value in ["1", "true", "YES", "on"] {
            let cfg = TerminalEnvConfig::from_iter(vec![("CLAUDE_CODE_DISABLE_MOUSE", value)]);
            assert!(
                cfg.disable_mouse,
                "value {:?} should enable disable_mouse",
                value
            );
        }
    }

    #[test]
    fn scroll_speed_parses_and_clamps() {
        let cfg = TerminalEnvConfig::from_iter(vec![("CLAUDE_CODE_SCROLL_SPEED", "12")]);
        assert_eq!(cfg.scroll_speed, 12);

        let cfg_hi = TerminalEnvConfig::from_iter(vec![("CLAUDE_CODE_SCROLL_SPEED", "9999")]);
        assert_eq!(cfg_hi.scroll_speed, TerminalEnvConfig::MAX_SCROLL_SPEED);

        let cfg_zero = TerminalEnvConfig::from_iter(vec![("CLAUDE_CODE_SCROLL_SPEED", "0")]);
        assert_eq!(cfg_zero.scroll_speed, TerminalEnvConfig::MIN_SCROLL_SPEED);
    }

    #[test]
    fn garbage_values_fall_back_to_defaults() {
        let cfg = TerminalEnvConfig::from_iter(vec![
            ("CLAUDE_CODE_NO_FLICKER", "maybe"),
            ("CLAUDE_CODE_DISABLE_MOUSE", ""),
            ("CLAUDE_CODE_SCROLL_SPEED", "fast"),
        ]);
        assert_eq!(cfg, TerminalEnvConfig::default());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let cfg = TerminalEnvConfig::from_iter(vec![("UNRELATED", "1")]);
        assert_eq!(cfg, TerminalEnvConfig::default());
    }

    #[test]
    fn parse_editor_command_supports_bare_programs() {
        let parsed = parse_editor_command("nvim").expect("editor command");
        assert_eq!(parsed.program, "nvim");
        assert_eq!(parsed.arguments, None);
        assert!(parsed.transcript_export_supported());
    }

    #[test]
    fn parse_editor_command_supports_quoted_program_paths() {
        let parsed = parse_editor_command("\"C:\\Program Files\\Neovim\\bin\\nvim-qt.exe\"")
            .expect("editor command");
        assert_eq!(
            parsed.program,
            "C:\\Program Files\\Neovim\\bin\\nvim-qt.exe"
        );
        assert_eq!(parsed.arguments, None);
        assert!(parsed.transcript_export_supported());
    }

    #[test]
    fn parse_editor_command_detects_argument_suffixes() {
        let parsed = parse_editor_command("code --wait").expect("editor command");
        assert_eq!(parsed.program, "code");
        assert_eq!(parsed.arguments.as_deref(), Some("--wait"));
        assert!(!parsed.transcript_export_supported());
    }

    #[test]
    fn parse_editor_command_detects_quoted_program_with_args() {
        let parsed = parse_editor_command("\"C:\\Program Files\\Neovim\\bin\\nvim-qt.exe\" -f")
            .expect("editor command");
        assert_eq!(
            parsed.program,
            "C:\\Program Files\\Neovim\\bin\\nvim-qt.exe"
        );
        assert_eq!(parsed.arguments.as_deref(), Some("-f"));
        assert!(!parsed.transcript_export_supported());
    }

    #[test]
    fn parse_editor_command_rejects_unterminated_quotes() {
        assert!(parse_editor_command("\"C:\\broken path").is_none());
    }
}

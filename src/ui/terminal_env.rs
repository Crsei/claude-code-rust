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
//! | `CLAUDE_CODE_DISABLE_MOUSE` | `1` prevents the TUI from enabling mouse capture.  |
//! |                             | cc-rust currently doesn't grab the mouse, so this  |
//! |                             | is mostly a declaration of intent — future code    |
//! |                             | that wants mouse wheel scroll has to check this.   |
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
            scroll_speed: 5,
        }
    }
}

impl TerminalEnvConfig {
    /// Default scroll speed when no override is set. Exposed publicly
    /// so `/terminal-setup` / tests can surface the same number that
    /// `Default::default()` seeds.
    #[allow(dead_code)]
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
                        cfg.scroll_speed =
                            n.clamp(Self::MIN_SCROLL_SPEED, Self::MAX_SCROLL_SPEED);
                    }
                }
                _ => {}
            }
        }
        cfg
    }
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
            let cfg =
                TerminalEnvConfig::from_iter(vec![("CLAUDE_CODE_DISABLE_MOUSE", value)]);
            assert!(cfg.disable_mouse, "value {:?} should enable disable_mouse", value);
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
}

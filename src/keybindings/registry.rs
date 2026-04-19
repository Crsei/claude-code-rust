//! Effective keybinding registry — merged default + user bindings, with
//! context-aware lookup and mtime-polled hot reload.
//!
//! Resolution order:
//!   1. Exact chord match in the requested context.
//!   2. Fallback to the `Global` context (unless request was already Global).
//!   3. Misses return `None`.
//!
//! The registry holds an `Arc<RwLock<State>>` so the Rust TUI can share one
//! instance across threads. `resolve()` transparently re-reads the user
//! file when its `mtime` changes; parse failures are logged and the last
//! good config is retained.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;

use super::action::Action;
use super::config::{BindingValue, KeybindingsConfigError, UserBindings};
use super::context::Context;
use super::defaults::iter_parsed;
use super::keystroke::{Chord, Keystroke};

/// A prefix match that waits for the next keystroke in a chord.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Chord fully matched → here's the action.
    Action(Action),
    /// Pressed keystroke matches the prefix of some chord; wait for the
    /// next keystroke.
    Pending,
    /// No binding matches.
    None,
}

/// Inner registry state — one effective map.
#[derive(Debug, Default)]
struct State {
    /// `Context → (Chord → Action)` map.
    effective: HashMap<Context, HashMap<Chord, Action>>,
    /// Last user-file mtime we observed.
    last_mtime: Option<SystemTime>,
    /// Path to the user file (may or may not exist).
    user_path: Option<PathBuf>,
    /// Parse issues from the last attempted reload (empty means all good).
    last_issues: Vec<String>,
}

/// Shared registry handle. Cheap to clone (`Arc`).
#[derive(Debug, Clone, Default)]
pub struct KeybindingRegistry {
    inner: Arc<RwLock<State>>,
}

impl KeybindingRegistry {
    /// Build a registry populated with the default bindings only.
    pub fn with_defaults() -> Self {
        let mut state = State::default();
        install_defaults(&mut state.effective);
        Self {
            inner: Arc::new(RwLock::new(state)),
        }
    }

    /// Build with defaults and an optional user file path (not yet read).
    pub fn with_user_path(user_path: Option<PathBuf>) -> Self {
        let me = Self::with_defaults();
        {
            let mut s = me.inner.write();
            s.user_path = user_path;
        }
        // Best-effort initial load; parse issues are kept on the state.
        let _ = me.reload();
        me
    }

    /// Is a user file path configured?
    pub fn has_user_path(&self) -> bool {
        self.inner.read().user_path.is_some()
    }

    /// Return the configured user path (for `/keybindings`).
    pub fn user_path(&self) -> Option<PathBuf> {
        self.inner.read().user_path.clone()
    }

    /// Force a reload now. Returns `Ok(())` when the reload succeeds (or the
    /// file doesn't exist) and `Err` when the user file exists but can't be
    /// parsed; in that case the previous effective set is retained.
    pub fn reload(&self) -> Result<(), KeybindingsConfigError> {
        let mut state = self.inner.write();
        let Some(path) = state.user_path.clone() else {
            return Ok(());
        };
        let mtime = fs::metadata(&path).and_then(|m| m.modified()).ok();
        state.last_mtime = mtime;

        match fs::read_to_string(&path) {
            Ok(text) => {
                let user = UserBindings::parse_json(&text)?;
                let mut effective = HashMap::new();
                install_defaults(&mut effective);
                apply_user(&mut effective, &user);
                state.effective = effective;
                state.last_issues.clear();
                Ok(())
            }
            Err(_) => {
                // Missing file is not an error; unreadable is logged and we
                // fall back to defaults.
                let mut effective = HashMap::new();
                install_defaults(&mut effective);
                state.effective = effective;
                Ok(())
            }
        }
    }

    /// Check the user file's mtime and reload if it changed.
    ///
    /// On parse failure, the error is stashed (see [`Self::last_issues`])
    /// and the previous effective config is kept so the UI stays usable.
    pub fn refresh_if_changed(&self) {
        let path = self.inner.read().user_path.clone();
        let Some(path) = path else { return };
        let new_mtime = fs::metadata(&path).and_then(|m| m.modified()).ok();
        let changed = {
            let state = self.inner.read();
            state.last_mtime != new_mtime
        };
        if !changed {
            return;
        }
        if let Err(e) = self.reload() {
            let mut state = self.inner.write();
            state.last_issues = vec![e.to_string()];
            state.last_mtime = new_mtime; // avoid flapping on repeated errors
            tracing::warn!(error = %e, "keybindings.json parse failed; keeping previous config");
        }
    }

    /// Return issues from the most recent reload attempt (empty if clean).
    pub fn last_issues(&self) -> Vec<String> {
        self.inner.read().last_issues.clone()
    }

    /// Resolve a single-keystroke lookup. For chords, use [`Self::resolve_chord`].
    pub fn resolve_single(&self, ctx: Context, stroke: &Keystroke) -> Resolution {
        // Auto-refresh if the user file changed.
        self.refresh_if_changed();

        let state = self.inner.read();
        // Try the specific context first, then Global.
        for &lookup_ctx in &[ctx, Context::Global] {
            if let Some(map) = state.effective.get(&lookup_ctx) {
                // Exact single-stroke match
                for (chord, action) in map {
                    if chord.is_single() && chord.strokes()[0] == *stroke {
                        return Resolution::Action(action.clone());
                    }
                }
                // Prefix match → pending
                for chord in map.keys() {
                    if chord.strokes().len() > 1 && chord.strokes()[0] == *stroke {
                        return Resolution::Pending;
                    }
                }
            }
            if ctx == Context::Global {
                break;
            }
        }
        Resolution::None
    }

    /// Resolve a complete chord (one or more keystrokes).
    pub fn resolve_chord(&self, ctx: Context, chord: &Chord) -> Resolution {
        self.refresh_if_changed();

        let state = self.inner.read();
        for &lookup_ctx in &[ctx, Context::Global] {
            if let Some(map) = state.effective.get(&lookup_ctx) {
                if let Some(action) = map.get(chord) {
                    return Resolution::Action(action.clone());
                }
            }
            if ctx == Context::Global {
                break;
            }
        }
        Resolution::None
    }

    /// Iterate all effective bindings for introspection (`/keybindings list`,
    /// help pages, tests).
    pub fn all_bindings(&self) -> Vec<(Context, Chord, Action)> {
        let state = self.inner.read();
        let mut out = Vec::new();
        for (ctx, map) in &state.effective {
            for (chord, action) in map {
                out.push((*ctx, chord.clone(), action.clone()));
            }
        }
        out.sort_by(|a, b| {
            (a.0.as_str(), a.1.display()).cmp(&(b.0.as_str(), b.1.display()))
        });
        out
    }

    /// List all bindings for a specific action across every context.
    pub fn bindings_for(&self, action: &Action) -> Vec<(Context, Chord)> {
        let state = self.inner.read();
        let mut out = Vec::new();
        for (ctx, map) in &state.effective {
            for (chord, a) in map {
                if a == action {
                    out.push((*ctx, chord.clone()));
                }
            }
        }
        out
    }
}

fn install_defaults(map: &mut HashMap<Context, HashMap<Chord, Action>>) {
    for (ctx, chord, action) in iter_parsed() {
        map.entry(ctx).or_default().insert(chord, action);
    }
}

fn apply_user(
    effective: &mut HashMap<Context, HashMap<Chord, Action>>,
    user: &UserBindings,
) {
    for (ctx, entries) in &user.per_context {
        let ctx_map = effective.entry(*ctx).or_default();
        // First pass: handle unbinds. Support unbinding a specific chord,
        // and also unbinding all chords that share a prefix (spec).
        let mut to_unbind: HashSet<Chord> = HashSet::new();
        for (chord, value) in entries {
            if matches!(value, BindingValue::Unbind) {
                to_unbind.insert(chord.clone());
            }
        }
        if !to_unbind.is_empty() {
            ctx_map.retain(|c, _| !to_unbind.contains(c));
        }
        // Second pass: apply binds (overrides defaults).
        for (chord, value) in entries {
            if let BindingValue::Bind(action) = value {
                ctx_map.insert(chord.clone(), action.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_registry_has_defaults() {
        let reg = KeybindingRegistry::with_defaults();
        let all = reg.all_bindings();
        assert!(all.len() > 30, "expected many defaults, got {}", all.len());
    }

    #[test]
    fn resolve_global_ctrl_c() {
        let reg = KeybindingRegistry::with_defaults();
        let stroke = Keystroke::parse("ctrl+c").unwrap();
        let r = reg.resolve_single(Context::Chat, &stroke);
        assert_eq!(
            r,
            Resolution::Action(Action::new_static("app:interrupt"))
        );
    }

    #[test]
    fn resolve_context_specific_enter_submits() {
        let reg = KeybindingRegistry::with_defaults();
        let stroke = Keystroke::parse("enter").unwrap();
        let r = reg.resolve_single(Context::Chat, &stroke);
        assert_eq!(r, Resolution::Action(Action::new_static("chat:submit")));
    }

    #[test]
    fn resolve_chord_prefix_returns_pending() {
        let reg = KeybindingRegistry::with_defaults();
        let stroke = Keystroke::parse("ctrl+x").unwrap();
        let r = reg.resolve_single(Context::Chat, &stroke);
        assert_eq!(r, Resolution::Pending);
    }

    #[test]
    fn resolve_full_chord_returns_action() {
        let reg = KeybindingRegistry::with_defaults();
        let chord = Chord::parse("ctrl+x ctrl+k").unwrap();
        let r = reg.resolve_chord(Context::Chat, &chord);
        assert_eq!(
            r,
            Resolution::Action(Action::new_static("chat:killAgents"))
        );
    }

    #[test]
    fn user_can_override_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keybindings.json");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"{{
                "bindings": [
                    {{"context": "Chat", "bindings": {{"enter": "chat:newline"}}}}
                ]
            }}"#
        )
        .unwrap();
        drop(f);

        let reg = KeybindingRegistry::with_user_path(Some(path));
        let stroke = Keystroke::parse("enter").unwrap();
        let r = reg.resolve_single(Context::Chat, &stroke);
        assert_eq!(r, Resolution::Action(Action::new_static("chat:newline")));
    }

    #[test]
    fn user_can_unbind_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keybindings.json");
        std::fs::write(
            &path,
            r#"{
                "bindings": [
                    {"context": "Chat", "bindings": {"ctrl+l": null}}
                ]
            }"#,
        )
        .unwrap();

        let reg = KeybindingRegistry::with_user_path(Some(path));
        let stroke = Keystroke::parse("ctrl+l").unwrap();
        let r = reg.resolve_single(Context::Chat, &stroke);
        assert_eq!(r, Resolution::None);
    }

    #[test]
    fn missing_user_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.json");
        let reg = KeybindingRegistry::with_user_path(Some(path));
        let stroke = Keystroke::parse("ctrl+c").unwrap();
        assert_eq!(
            reg.resolve_single(Context::Global, &stroke),
            Resolution::Action(Action::new_static("app:interrupt"))
        );
    }

    #[test]
    fn hot_reload_picks_up_changes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keybindings.json");
        std::fs::write(
            &path,
            r#"{"bindings":[{"context":"Chat","bindings":{"enter":"chat:newline"}}]}"#,
        )
        .unwrap();
        // Set mtime to an old value so the future rewrite is detected
        let reg = KeybindingRegistry::with_user_path(Some(path.clone()));

        // Sanity: initial override works
        let stroke = Keystroke::parse("enter").unwrap();
        assert_eq!(
            reg.resolve_single(Context::Chat, &stroke),
            Resolution::Action(Action::new_static("chat:newline"))
        );

        // Sleep briefly so the mtime actually advances on coarse timestamp
        // systems, then rewrite the file.
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(
            &path,
            r#"{"bindings":[{"context":"Chat","bindings":{"enter":"chat:submit"}}]}"#,
        )
        .unwrap();

        // Resolve triggers refresh_if_changed → rereads → picks up submit.
        assert_eq!(
            reg.resolve_single(Context::Chat, &stroke),
            Resolution::Action(Action::new_static("chat:submit"))
        );
    }

    #[test]
    fn parse_failure_retains_previous_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keybindings.json");
        std::fs::write(
            &path,
            r#"{"bindings":[{"context":"Chat","bindings":{"enter":"chat:newline"}}]}"#,
        )
        .unwrap();
        let reg = KeybindingRegistry::with_user_path(Some(path.clone()));

        let stroke = Keystroke::parse("enter").unwrap();
        assert_eq!(
            reg.resolve_single(Context::Chat, &stroke),
            Resolution::Action(Action::new_static("chat:newline"))
        );

        // Now write invalid JSON — should NOT break the registry.
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "{ this is not json").unwrap();
        assert_eq!(
            reg.resolve_single(Context::Chat, &stroke),
            Resolution::Action(Action::new_static("chat:newline")),
            "bad JSON must not clobber previous config"
        );
        let issues = reg.last_issues();
        assert!(!issues.is_empty(), "issues should be recorded");
    }

    #[test]
    fn bindings_for_action_covers_multiple_contexts() {
        let reg = KeybindingRegistry::with_defaults();
        let cancel = Action::new_static("chat:cancel");
        let hits = reg.bindings_for(&cancel);
        assert!(
            hits.iter().any(|(c, _)| *c == Context::Chat),
            "chat:cancel should be bound in Chat context"
        );
    }
}

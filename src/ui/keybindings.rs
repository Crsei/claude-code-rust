//! Keybinding registry — maps key events to named actions.
//!
//! Provides a configurable keybinding system with:
//! - Default keybindings for common operations
//! - User-customizable overrides
//! - Context-aware binding resolution (global vs input-focused)
//!
//! Corresponds to TypeScript: keybindings/ (14 files)

#![allow(unused)]

use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ---------------------------------------------------------------------------
// Action definitions
// ---------------------------------------------------------------------------

/// Named actions that can be triggered by keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    // -- Global actions --
    Quit,
    ForceQuit,
    Cancel,
    Help,
    ToggleVerbose,
    ToggleFastMode,

    // -- Navigation --
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,

    // -- Input actions --
    Submit,
    ClearInput,
    CursorLeft,
    CursorRight,
    CursorHome,
    CursorEnd,
    DeleteBack,
    DeleteForward,
    DeleteWord,
    KillToEnd,

    // -- History --
    HistoryPrev,
    HistoryNext,

    // -- Commands --
    CompactHistory,
    ClearMessages,
    ShowCost,

    // -- Vim --
    EnterVimNormal,
}

impl Action {
    /// Human-readable description for this action.
    pub fn description(&self) -> &'static str {
        match self {
            Action::Quit => "Quit the application",
            Action::ForceQuit => "Force quit without confirmation",
            Action::Cancel => "Cancel current operation",
            Action::Help => "Show help",
            Action::ToggleVerbose => "Toggle verbose mode",
            Action::ToggleFastMode => "Toggle fast mode",
            Action::ScrollUp => "Scroll up one line",
            Action::ScrollDown => "Scroll down one line",
            Action::ScrollPageUp => "Scroll up one page",
            Action::ScrollPageDown => "Scroll down one page",
            Action::ScrollToTop => "Scroll to top",
            Action::ScrollToBottom => "Scroll to bottom",
            Action::Submit => "Submit input",
            Action::ClearInput => "Clear input line",
            Action::CursorLeft => "Move cursor left",
            Action::CursorRight => "Move cursor right",
            Action::CursorHome => "Move cursor to start",
            Action::CursorEnd => "Move cursor to end",
            Action::DeleteBack => "Delete character before cursor",
            Action::DeleteForward => "Delete character after cursor",
            Action::DeleteWord => "Delete word before cursor",
            Action::KillToEnd => "Delete to end of line",
            Action::HistoryPrev => "Previous input history",
            Action::HistoryNext => "Next input history",
            Action::CompactHistory => "Compact message history",
            Action::ClearMessages => "Clear all messages",
            Action::ShowCost => "Show token cost",
            Action::EnterVimNormal => "Enter vim normal mode",
        }
    }
}

// ---------------------------------------------------------------------------
// Key binding
// ---------------------------------------------------------------------------

/// A concrete key combination: modifier flags + key code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyBind {
    pub modifiers: KeyModifiers,
    pub code: KeyCode,
}

impl KeyBind {
    pub const fn new(modifiers: KeyModifiers, code: KeyCode) -> Self {
        Self { modifiers, code }
    }

    /// Create from a `KeyEvent`.
    pub fn from_event(event: &KeyEvent) -> Self {
        Self {
            modifiers: event.modifiers,
            code: event.code,
        }
    }

    /// Human-readable display of this key combination.
    pub fn display(&self) -> String {
        let mut parts = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }

        let key_name = match self.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Enter => "Enter".into(),
            KeyCode::Esc => "Esc".into(),
            KeyCode::Backspace => "Backspace".into(),
            KeyCode::Delete => "Delete".into(),
            KeyCode::Left => "Left".into(),
            KeyCode::Right => "Right".into(),
            KeyCode::Up => "Up".into(),
            KeyCode::Down => "Down".into(),
            KeyCode::Home => "Home".into(),
            KeyCode::End => "End".into(),
            KeyCode::PageUp => "PageUp".into(),
            KeyCode::PageDown => "PageDown".into(),
            KeyCode::Tab => "Tab".into(),
            KeyCode::F(n) => format!("F{}", n),
            _ => "?".into(),
        };

        parts.push(&key_name);
        // We need to own the string to return it
        let mut result_parts: Vec<String> = Vec::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            result_parts.push("Ctrl".into());
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            result_parts.push("Alt".into());
        }
        if self.modifiers.contains(KeyModifiers::SHIFT) {
            result_parts.push("Shift".into());
        }
        result_parts.push(key_name);
        result_parts.join("+")
    }
}

// ---------------------------------------------------------------------------
// Binding context
// ---------------------------------------------------------------------------

/// Context in which keybindings are resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingContext {
    /// Global: always active (e.g. Ctrl+C to quit).
    Global,
    /// Input: active when the input field is focused.
    Input,
    /// Scrolling: active when message viewport is focused.
    Scroll,
}

// ---------------------------------------------------------------------------
// Keybinding registry
// ---------------------------------------------------------------------------

/// Maps key combinations to actions, organized by context.
pub struct KeybindingRegistry {
    bindings: HashMap<BindingContext, HashMap<KeyBind, Action>>,
}

impl KeybindingRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Create a registry with the default keybindings.
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register_defaults();
        reg
    }

    /// Register a single binding.
    pub fn bind(&mut self, context: BindingContext, key: KeyBind, action: Action) {
        self.bindings
            .entry(context)
            .or_default()
            .insert(key, action);
    }

    /// Remove a binding.
    pub fn unbind(&mut self, context: BindingContext, key: &KeyBind) -> Option<Action> {
        self.bindings
            .get_mut(&context)
            .and_then(|map| map.remove(key))
    }

    /// Look up an action for a key event in the given context.
    ///
    /// Falls back to `Global` context if no match in the specific context.
    pub fn resolve(&self, context: BindingContext, event: &KeyEvent) -> Option<Action> {
        let key = KeyBind::from_event(event);

        // Check specific context first
        if context != BindingContext::Global {
            if let Some(action) = self
                .bindings
                .get(&context)
                .and_then(|map| map.get(&key))
            {
                return Some(*action);
            }
        }

        // Fall back to global
        self.bindings
            .get(&BindingContext::Global)
            .and_then(|map| map.get(&key))
            .copied()
    }

    /// Get all bindings for a given action (across all contexts).
    pub fn bindings_for_action(&self, action: Action) -> Vec<(BindingContext, KeyBind)> {
        let mut result = Vec::new();
        for (ctx, map) in &self.bindings {
            for (key, act) in map {
                if *act == action {
                    result.push((*ctx, *key));
                }
            }
        }
        result
    }

    /// List all bindings, organized by context.
    pub fn list_all(&self) -> Vec<(BindingContext, KeyBind, Action)> {
        let mut result = Vec::new();
        for (ctx, map) in &self.bindings {
            for (key, action) in map {
                result.push((*ctx, *key, *action));
            }
        }
        result.sort_by_key(|(ctx, _, _)| *ctx as u8);
        result
    }

    /// Register the default keybinding set.
    fn register_defaults(&mut self) {
        use Action::*;
        use BindingContext::*;
        use KeyCode::*;

        let ctrl = KeyModifiers::CONTROL;
        let none = KeyModifiers::NONE;

        // -- Global --
        self.bind(Global, KeyBind::new(ctrl, Char('c')), Cancel);
        self.bind(Global, KeyBind::new(ctrl, Char('d')), Quit);
        self.bind(Global, KeyBind::new(none, KeyCode::Esc), Cancel);

        // -- Input --
        self.bind(Input, KeyBind::new(none, Enter), Submit);
        self.bind(Input, KeyBind::new(ctrl, Char('u')), ClearInput);
        self.bind(Input, KeyBind::new(ctrl, Char('a')), CursorHome);
        self.bind(Input, KeyBind::new(ctrl, Char('e')), CursorEnd);
        self.bind(Input, KeyBind::new(ctrl, Char('w')), DeleteWord);
        self.bind(Input, KeyBind::new(ctrl, Char('k')), KillToEnd);
        self.bind(Input, KeyBind::new(none, Left), CursorLeft);
        self.bind(Input, KeyBind::new(none, Right), CursorRight);
        self.bind(Input, KeyBind::new(none, Home), CursorHome);
        self.bind(Input, KeyBind::new(none, End), CursorEnd);
        self.bind(Input, KeyBind::new(none, Backspace), DeleteBack);
        self.bind(Input, KeyBind::new(none, Delete), DeleteForward);
        self.bind(Input, KeyBind::new(none, Up), HistoryPrev);
        self.bind(Input, KeyBind::new(none, Down), HistoryNext);

        // -- Scroll --
        self.bind(Scroll, KeyBind::new(none, Up), ScrollUp);
        self.bind(Scroll, KeyBind::new(none, Down), ScrollDown);
        self.bind(Scroll, KeyBind::new(none, PageUp), ScrollPageUp);
        self.bind(Scroll, KeyBind::new(none, PageDown), ScrollPageDown);
        self.bind(Scroll, KeyBind::new(ctrl, Char('g')), ScrollToTop);
        self.bind(Scroll, KeyBind::new(none, End), ScrollToBottom);
    }
}

impl Default for KeybindingRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bindings_exist() {
        let reg = KeybindingRegistry::with_defaults();
        let all = reg.list_all();
        assert!(all.len() >= 15, "should have many default bindings");
    }

    #[test]
    fn test_resolve_global() {
        let reg = KeybindingRegistry::with_defaults();
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(
            reg.resolve(BindingContext::Global, &event),
            Some(Action::Cancel)
        );
    }

    #[test]
    fn test_resolve_fallback_to_global() {
        let reg = KeybindingRegistry::with_defaults();
        // Ctrl+C is global, should resolve even in Input context
        let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(
            reg.resolve(BindingContext::Input, &event),
            Some(Action::Cancel)
        );
    }

    #[test]
    fn test_resolve_context_specific() {
        let reg = KeybindingRegistry::with_defaults();
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(
            reg.resolve(BindingContext::Input, &event),
            Some(Action::Submit)
        );
        // Enter in Scroll context should not match (not bound there)
        assert_eq!(reg.resolve(BindingContext::Scroll, &event), None);
    }

    #[test]
    fn test_custom_binding() {
        let mut reg = KeybindingRegistry::with_defaults();
        let key = KeyBind::new(KeyModifiers::CONTROL, KeyCode::Char('t'));
        reg.bind(BindingContext::Global, key, Action::ToggleFastMode);

        let event = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL);
        assert_eq!(
            reg.resolve(BindingContext::Global, &event),
            Some(Action::ToggleFastMode)
        );
    }

    #[test]
    fn test_unbind() {
        let mut reg = KeybindingRegistry::with_defaults();
        let key = KeyBind::new(KeyModifiers::CONTROL, KeyCode::Char('d'));
        let removed = reg.unbind(BindingContext::Global, &key);
        assert_eq!(removed, Some(Action::Quit));

        let event = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        assert_eq!(reg.resolve(BindingContext::Global, &event), None);
    }

    #[test]
    fn test_bindings_for_action() {
        let reg = KeybindingRegistry::with_defaults();
        let cancel_bindings = reg.bindings_for_action(Action::Cancel);
        assert!(
            cancel_bindings.len() >= 2,
            "Cancel should be bound to Ctrl+C and Esc"
        );
    }

    #[test]
    fn test_key_bind_display() {
        let key = KeyBind::new(KeyModifiers::CONTROL, KeyCode::Char('c'));
        assert_eq!(key.display(), "Ctrl+c");

        let key2 = KeyBind::new(KeyModifiers::NONE, KeyCode::Enter);
        assert_eq!(key2.display(), "Enter");

        let key3 = KeyBind::new(
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            KeyCode::Char('z'),
        );
        assert_eq!(key3.display(), "Ctrl+Shift+z");
    }

    #[test]
    fn test_no_match_returns_none() {
        let reg = KeybindingRegistry::with_defaults();
        let event = KeyEvent::new(KeyCode::F(12), KeyModifiers::NONE);
        assert_eq!(reg.resolve(BindingContext::Global, &event), None);
    }
}

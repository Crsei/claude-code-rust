//! Chat-widget adapter for the existing cc-rust [`super::app::App`].
//!
//! Upstream Codex keeps most TUI state in `chatwidget.rs`; cc-rust already has
//! that state in `app.rs`. This module provides the documented file boundary
//! without duplicating the full app implementation.

use crossterm::event::KeyEvent;

use cc_types::message::Message;

use super::app::{App, AppAction};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChatWidgetInit {
    pub model_name: Option<String>,
    pub backend_name: Option<String>,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub voice_enabled: bool,
    pub voice_language: Option<String>,
    pub voice_supported: bool,
}

pub struct ChatWidget {
    app: App,
}

impl ChatWidget {
    pub fn new(init: ChatWidgetInit) -> Self {
        let mut app = App::new();
        if let Some(model) = init.model_name {
            app.set_model_name(model);
        }
        if let Some(backend) = init.backend_name {
            app.set_backend_name(backend);
        }
        if let Some(session_id) = init.session_id {
            app.set_session_id(session_id);
        }
        if let Some(cwd) = init.cwd {
            app.set_cwd(cwd);
        }
        app.set_voice_settings(
            init.voice_enabled,
            init.voice_language.unwrap_or_else(|| "en".to_string()),
            init.voice_supported,
        );
        Self { app }
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn from_app(app: App) -> Self {
        Self { app }
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn app(&self) -> &App {
        &self.app
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn into_app(self) -> App {
        self.app
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn add_message(&mut self, message: Message) {
        self.app.add_message(message);
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn messages(&self) -> &[Message] {
        self.app.messages()
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn clear_messages(&mut self) {
        self.app.clear_messages();
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn set_streaming(&mut self, streaming: bool) {
        self.app.set_streaming(streaming);
    }

    #[allow(dead_code)] // Phase 1: upstream parity surface
    pub fn handle_key_event(&mut self, key: KeyEvent) -> AppAction {
        self.app.handle_key_event(key)
    }
}

impl Default for ChatWidget {
    fn default() -> Self {
        Self::new(ChatWidgetInit::default())
    }
}

#[allow(dead_code)] // Phase 1: upstream parity surface
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayKind {
    Full,
    VisibleOnly,
}

#[allow(dead_code)] // Phase 1: upstream parity surface
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMessage {
    pub text: String,
}

impl From<String> for UserMessage {
    fn from(text: String) -> Self {
        Self { text }
    }
}

impl From<&str> for UserMessage {
    fn from(text: &str) -> Self {
        Self {
            text: text.to_string(),
        }
    }
}

#[allow(dead_code)] // Phase 1: upstream parity surface
pub fn create_initial_user_message(text: impl Into<String>) -> UserMessage {
    UserMessage { text: text.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_existing_app_state() {
        let widget = ChatWidget::new(ChatWidgetInit {
            model_name: Some("test-model".to_string()),
            backend_name: Some("native".to_string()),
            session_id: Some("session".to_string()),
            cwd: Some(".".to_string()),
            ..ChatWidgetInit::default()
        });
        assert!(widget.messages().is_empty());
        assert!(widget.app().is_dirty());
    }
}

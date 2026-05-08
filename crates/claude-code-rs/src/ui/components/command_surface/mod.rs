//! Interactive slash-command surfaces for the Rust TUI.

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use crate::ipc::subsystem_types::LspRecommendationPayload;
use crate::types::app_state::AppState;

mod adapters;
mod surfaces;

pub use surfaces::agents::AgentsSurface;
pub use surfaces::config::ConfigSurface;
pub use surfaces::diff::DiffSurface;
pub use surfaces::hooks::HooksSurface;
pub use surfaces::login::LoginSurface;
pub use surfaces::lsp_recommendation::LspRecommendationSurface;
pub use surfaces::mcp::McpSurface;
pub use surfaces::memory::MemorySurface;
pub use surfaces::remote::RemoteSurface;
pub use surfaces::sandbox::SandboxSurface;
pub use surfaces::skills::SkillsSurface;
pub use surfaces::tasks::TasksSurface;
pub use surfaces::team::TeamSurface;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSurfaceOutcome {
    None,
    Close,
    FillPrompt(String),
    Submit(String),
    LspRecommendationResponse {
        request_id: String,
        plugin_name: String,
        decision: String,
        install_prompt: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandSurface {
    Agents(AgentsSurface),
    Config(ConfigSurface),
    Diff(DiffSurface),
    Hooks(HooksSurface),
    Login(LoginSurface),
    Mcp(McpSurface),
    Memory(MemorySurface),
    Remote(RemoteSurface),
    Sandbox(SandboxSurface),
    Skills(SkillsSurface),
    Tasks(TasksSurface),
    Team(TeamSurface),
    LspRecommendation(LspRecommendationSurface),
}

impl CommandSurface {
    pub fn for_slash_command(name: &str, args: &str, state: &AppState, cwd: &Path) -> Option<Self> {
        if !args.trim().is_empty() {
            return None;
        }

        match name {
            "agents" => Some(Self::Agents(AgentsSurface::new(cwd))),
            "config" => Some(Self::Config(ConfigSurface::new(state))),
            "diff" => Some(Self::Diff(DiffSurface::new(cwd))),
            "hooks" => Some(Self::Hooks(HooksSurface::new(&state.hooks))),
            "login" => Some(Self::Login(LoginSurface::new())),
            "mcp" => Some(Self::Mcp(McpSurface::new(cwd))),
            "memory" => Some(Self::Memory(MemorySurface::new(cwd))),
            "remote" => Some(Self::Remote(RemoteSurface::new())),
            "sandbox" => Some(Self::Sandbox(SandboxSurface::new(state))),
            "skills" => Some(Self::Skills(SkillsSurface::new())),
            "tasks" => Some(Self::Tasks(TasksSurface::new())),
            "team" | "teams" => Some(Self::Team(TeamSurface::new(state))),
            _ => None,
        }
    }

    pub fn lsp_recommendation(payload: LspRecommendationPayload) -> Self {
        Self::LspRecommendation(LspRecommendationSurface::new(payload))
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Agents(_) => "Agents",
            Self::Config(_) => "Config",
            Self::Diff(_) => "Diff",
            Self::Hooks(_) => "Hooks",
            Self::Login(_) => "Login",
            Self::Mcp(_) => "MCP",
            Self::Memory(_) => "Memory",
            Self::Remote(_) => "Remote",
            Self::Sandbox(_) => "Sandbox",
            Self::Skills(_) => "Skills",
            Self::Tasks(_) => "Tasks",
            Self::Team(_) => "Team",
            Self::LspRecommendation(_) => "LSP Plugin Recommendation",
        }
    }

    pub fn render(&self) -> String {
        match self {
            Self::Agents(surface) => surface.render(),
            Self::Config(surface) => surface.render(),
            Self::Diff(surface) => surface.render(),
            Self::Hooks(surface) => surface.render(),
            Self::Login(surface) => surface.render(),
            Self::Mcp(surface) => surface.render(),
            Self::Memory(surface) => surface.render(),
            Self::Remote(surface) => surface.render(),
            Self::Sandbox(surface) => surface.render(),
            Self::Skills(surface) => surface.render(),
            Self::Tasks(surface) => surface.render(),
            Self::Team(surface) => surface.render(),
            Self::LspRecommendation(surface) => surface.render(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> CommandSurfaceOutcome {
        if key.kind != KeyEventKind::Press {
            return CommandSurfaceOutcome::None;
        }
        if matches!(key.code, KeyCode::Esc) {
            if let Self::LspRecommendation(surface) = self {
                return surface.cancel();
            }
            return CommandSurfaceOutcome::Close;
        }

        match self {
            Self::Agents(surface) => surface.handle_key(key),
            Self::Config(surface) => surface.handle_key(key),
            Self::Diff(surface) => surface.handle_key(key),
            Self::Hooks(surface) => surface.handle_key(key),
            Self::Login(surface) => surface.handle_key(key),
            Self::Mcp(surface) => surface.handle_key(key),
            Self::Memory(surface) => surface.handle_key(key),
            Self::Remote(surface) => surface.handle_key(key),
            Self::Sandbox(surface) => surface.handle_key(key),
            Self::Skills(surface) => surface.handle_key(key),
            Self::Tasks(surface) => surface.handle_key(key),
            Self::Team(surface) => surface.handle_key(key),
            Self::LspRecommendation(surface) => surface.handle_key(key),
        }
    }
}

fn render_tabs(labels: &[impl AsRef<str>], selected: usize) -> String {
    labels
        .iter()
        .enumerate()
        .map(|(idx, label)| {
            let label = label.as_ref();
            if idx == selected {
                format!("[{label}]")
            } else {
                format!(" {label} ")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn cycle_index(current: usize, len: usize, direction: isize) -> usize {
    if len == 0 {
        return 0;
    }
    if direction < 0 {
        if current == 0 {
            len - 1
        } else {
            current - 1
        }
    } else {
        (current + 1) % len
    }
}

#[cfg(test)]
mod tests;

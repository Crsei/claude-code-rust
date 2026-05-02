//! Full-build feature panel registry.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeaturePanelKind {
    Mcp,
    Agents,
    Teams,
    Lsp,
    Settings,
    Sandbox,
    Plugins,
    Skills,
    Tasks,
}

impl FeaturePanelKind {
    pub fn title(self) -> &'static str {
        match self {
            FeaturePanelKind::Mcp => "MCP",
            FeaturePanelKind::Agents => "Agents",
            FeaturePanelKind::Teams => "Teams",
            FeaturePanelKind::Lsp => "LSP",
            FeaturePanelKind::Settings => "Settings",
            FeaturePanelKind::Sandbox => "Sandbox",
            FeaturePanelKind::Plugins => "Plugins",
            FeaturePanelKind::Skills => "Skills",
            FeaturePanelKind::Tasks => "Tasks",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    Ready,
    NeedsConfig,
    Running,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeaturePanel {
    pub kind: FeaturePanelKind,
    pub state: PanelState,
    pub badge: usize,
}

pub fn default_panels() -> Vec<FeaturePanel> {
    [
        FeaturePanelKind::Mcp,
        FeaturePanelKind::Agents,
        FeaturePanelKind::Teams,
        FeaturePanelKind::Lsp,
        FeaturePanelKind::Settings,
        FeaturePanelKind::Sandbox,
        FeaturePanelKind::Plugins,
        FeaturePanelKind::Skills,
        FeaturePanelKind::Tasks,
    ]
    .into_iter()
    .map(|kind| FeaturePanel {
        kind,
        state: PanelState::Ready,
        badge: 0,
    })
    .collect()
}

pub fn render_panel_index(panels: &[FeaturePanel]) -> String {
    panels
        .iter()
        .map(|panel| {
            let badge = if panel.badge == 0 {
                String::new()
            } else {
                format!(" ({})", panel.badge)
            };
            format!("{:<9} {:?}{}", panel.kind.title(), panel.state, badge)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

//! LSP recommendation menu rendering.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRecommendation {
    pub language: String,
    pub server: String,
    pub install_command: Option<String>,
    pub reason: String,
    pub selected: bool,
}

impl LspRecommendation {
    pub fn new(language: impl Into<String>, server: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            server: server.into(),
            install_command: None,
            reason: String::new(),
            selected: false,
        }
    }
}

pub fn render_lsp_recommendation_menu(recommendations: &[LspRecommendation]) -> String {
    if recommendations.is_empty() {
        return "No language server recommendations".to_string();
    }

    let mut lines = vec!["Language servers".to_string()];
    for recommendation in recommendations {
        let marker = if recommendation.selected { ">" } else { " " };
        lines.push(format!(
            "{marker} {:<12} {}",
            recommendation.language, recommendation.server
        ));
        if !recommendation.reason.is_empty() {
            lines.push(format!("  reason: {}", recommendation.reason));
        }
        if let Some(command) = &recommendation.install_command {
            lines.push(format!("  install: {command}"));
        }
    }
    lines.join("\n")
}

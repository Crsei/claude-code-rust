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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LspRecommendationDecision {
    Yes,
    No,
    Never,
    Disable,
}

impl LspRecommendationDecision {
    pub fn value(self) -> &'static str {
        match self {
            LspRecommendationDecision::Yes => "yes",
            LspRecommendationDecision::No => "no",
            LspRecommendationDecision::Never => "never",
            LspRecommendationDecision::Disable => "disable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LspRecommendationPromptState {
    pub plugin_name: String,
    pub plugin_description: Option<String>,
    pub file_extension: String,
    pub selected_index: usize,
}

impl LspRecommendationPromptState {
    pub fn new(
        plugin_name: impl Into<String>,
        plugin_description: Option<String>,
        file_extension: impl Into<String>,
    ) -> Self {
        Self {
            plugin_name: plugin_name.into(),
            plugin_description,
            file_extension: file_extension.into(),
            selected_index: 0,
        }
    }

    pub fn move_next(&mut self) {
        self.selected_index = (self.selected_index + 1) % LSP_RECOMMENDATION_CHOICES.len();
    }

    pub fn move_prev(&mut self) {
        self.selected_index = if self.selected_index == 0 {
            LSP_RECOMMENDATION_CHOICES.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    pub fn selected_decision(&self) -> LspRecommendationDecision {
        LSP_RECOMMENDATION_CHOICES[self.selected_index].decision
    }

    pub fn render(&self) -> String {
        let mut lines = vec![
            "LSP Plugin Recommendation".to_string(),
            "LSP provides code intelligence like go-to-definition and error checking".to_string(),
            format!("Plugin: {}", self.plugin_name),
        ];
        if let Some(description) = &self.plugin_description {
            lines.push(description.clone());
        }
        lines.push(format!("Triggered by: {} files", self.file_extension));
        lines.push("Would you like to install this LSP plugin?".to_string());
        for (idx, choice) in LSP_RECOMMENDATION_CHOICES.iter().enumerate() {
            let marker = if idx == self.selected_index { ">" } else { " " };
            lines.push(format!("{marker} {}", choice.label(&self.plugin_name)));
        }
        lines.push("Enter choose | Esc no".to_string());
        lines.join("\n")
    }
}

#[derive(Debug, Clone, Copy)]
struct LspRecommendationChoice {
    decision: LspRecommendationDecision,
    label: &'static str,
}

impl LspRecommendationChoice {
    fn label(self, plugin_name: &str) -> String {
        match self.decision {
            LspRecommendationDecision::Yes => format!("Yes, install {plugin_name}"),
            LspRecommendationDecision::Never => format!("Never for {plugin_name}"),
            _ => self.label.to_string(),
        }
    }
}

const LSP_RECOMMENDATION_CHOICES: &[LspRecommendationChoice] = &[
    LspRecommendationChoice {
        decision: LspRecommendationDecision::Yes,
        label: "Yes",
    },
    LspRecommendationChoice {
        decision: LspRecommendationDecision::No,
        label: "No, not now",
    },
    LspRecommendationChoice {
        decision: LspRecommendationDecision::Never,
        label: "Never",
    },
    LspRecommendationChoice {
        decision: LspRecommendationDecision::Disable,
        label: "Disable all LSP recommendations",
    },
];

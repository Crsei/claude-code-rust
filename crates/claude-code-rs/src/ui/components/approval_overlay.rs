//! Typed approval and permission overlay primitives.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalKind {
    Bash { command: String },
    FileEdit { path: String },
    WebFetch { url: String },
    Mcp { server: String, action: String },
    UserInput { prompt: String },
    Fallback { summary: String },
}

impl ApprovalKind {
    pub fn title(&self) -> &'static str {
        match self {
            ApprovalKind::Bash { .. } => "Run command?",
            ApprovalKind::FileEdit { .. } => "Edit file?",
            ApprovalKind::WebFetch { .. } => "Fetch web content?",
            ApprovalKind::Mcp { .. } => "Allow MCP action?",
            ApprovalKind::UserInput { .. } => "Answer request?",
            ApprovalKind::Fallback { .. } => "Approve action?",
        }
    }

    pub fn subject(&self) -> String {
        match self {
            ApprovalKind::Bash { command } => command.clone(),
            ApprovalKind::FileEdit { path } => path.clone(),
            ApprovalKind::WebFetch { url } => url.clone(),
            ApprovalKind::Mcp { server, action } => format!("{server}: {action}"),
            ApprovalKind::UserInput { prompt } => prompt.clone(),
            ApprovalKind::Fallback { summary } => summary.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalChoice {
    AllowOnce,
    AllowAlways,
    Deny,
    EditRequest,
}

impl ApprovalChoice {
    pub fn label(self) -> &'static str {
        match self {
            ApprovalChoice::AllowOnce => "allow once",
            ApprovalChoice::AllowAlways => "always allow",
            ApprovalChoice::Deny => "deny",
            ApprovalChoice::EditRequest => "edit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalOverlay {
    pub kind: ApprovalKind,
    pub choices: Vec<ApprovalChoice>,
    pub selected: usize,
    pub fail_closed: bool,
}

impl ApprovalOverlay {
    pub fn new(kind: ApprovalKind) -> Self {
        Self {
            kind,
            choices: vec![ApprovalChoice::AllowOnce, ApprovalChoice::Deny],
            selected: 0,
            fail_closed: true,
        }
    }

    pub fn with_choices(mut self, choices: Vec<ApprovalChoice>) -> Self {
        self.choices = if choices.is_empty() {
            vec![ApprovalChoice::Deny]
        } else {
            choices
        };
        self.selected = self.selected.min(self.choices.len().saturating_sub(1));
        self
    }

    pub fn select(&mut self, selected: usize) {
        self.selected = selected.min(self.choices.len().saturating_sub(1));
    }

    pub fn selected_choice(&self) -> ApprovalChoice {
        self.choices
            .get(self.selected)
            .copied()
            .unwrap_or(ApprovalChoice::Deny)
    }

    pub fn render_lines(&self, width: usize) -> Vec<String> {
        let choices = self
            .choices
            .iter()
            .enumerate()
            .map(|(idx, choice)| {
                if idx == self.selected {
                    format!("[{}]", choice.label())
                } else {
                    choice.label().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("  ");

        vec![
            fit_line(self.kind.title(), width),
            fit_line(&self.kind.subject(), width),
            fit_line(&choices, width),
            format!(
                "default: {}",
                if self.fail_closed { "deny" } else { "allow" }
            ),
        ]
    }
}

fn fit_line(text: &str, width: usize) -> String {
    if width == 0 || text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".to_string();
    }
    let mut out: String = text.chars().take(width - 1).collect();
    out.push('…');
    out
}

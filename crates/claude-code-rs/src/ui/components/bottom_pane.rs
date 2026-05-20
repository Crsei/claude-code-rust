//! Bottom pane container and view stack.

use super::approval_overlay::ApprovalOverlay;
use super::chat_composer::ChatComposerState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BottomPaneView {
    Composer,
    Approval(ApprovalOverlay),
    Selection { title: String, count: usize },
    Status { message: String },
}

impl BottomPaneView {
    pub fn name(&self) -> &'static str {
        match self {
            BottomPaneView::Composer => "composer",
            BottomPaneView::Approval(_) => "approval",
            BottomPaneView::Selection { .. } => "selection",
            BottomPaneView::Status { .. } => "status",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BottomPane {
    pub composer: ChatComposerState,
    stack: Vec<BottomPaneView>,
}

impl BottomPane {
    pub fn new(composer: ChatComposerState) -> Self {
        Self {
            composer,
            stack: vec![BottomPaneView::Composer],
        }
    }

    pub fn push(&mut self, view: BottomPaneView) {
        self.stack.push(view);
    }
    pub fn pop(&mut self) -> Option<BottomPaneView> {
        if self.stack.len() <= 1 {
            return None;
        }
        self.stack.pop()
    }

    pub fn focused_view(&self) -> &BottomPaneView {
        self.stack.last().unwrap_or(&BottomPaneView::Composer)
    }

    pub fn render_lines(&self, width: usize) -> Vec<String> {
        let mut lines = vec![format!(
            "bottom-pane focus={} depth={}",
            self.focused_view().name(),
            self.stack.len()
        )];
        match self.focused_view() {
            BottomPaneView::Composer => lines.extend(self.composer.render_lines(width)),
            BottomPaneView::Approval(overlay) => lines.extend(overlay.render_lines(width)),
            BottomPaneView::Selection { title, count } => {
                lines.push(format!("select: {title} ({count} items)"));
            }
            BottomPaneView::Status { message } => lines.push(format!("status: {message}")),
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_supports_selection_status_and_pop() {
        let mut pane = BottomPane::new(ChatComposerState::new());
        pane.push(BottomPaneView::Selection {
            title: "Sessions".to_string(),
            count: 3,
        });
        assert_eq!(pane.focused_view().name(), "selection");
        assert!(pane.render_lines(80).join("\n").contains("Sessions"));

        pane.push(BottomPaneView::Status {
            message: "busy".to_string(),
        });
        assert_eq!(pane.focused_view().name(), "status");
        assert_eq!(pane.pop().unwrap().name(), "status");
        assert_eq!(pane.pop().unwrap().name(), "selection");
        assert!(pane.pop().is_none());
    }
}

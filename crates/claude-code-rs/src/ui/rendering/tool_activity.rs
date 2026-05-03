//! Tool activity and progress rendering.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolActivity {
    pub name: String,
    pub state: ToolState,
    pub summary: String,
    pub elapsed_ms: u64,
    pub output_lines: usize,
}

impl ToolActivity {
    pub fn compact_line(&self) -> String {
        format!(
            "{:?} {} - {} ({} lines, {}ms)",
            self.state, self.name, self.summary, self.output_lines, self.elapsed_ms
        )
    }

    pub fn transcript_block(&self) -> String {
        [
            format!("tool: {}", self.name),
            format!("state: {:?}", self.state),
            format!("summary: {}", self.summary),
            format!("output lines: {}", self.output_lines),
            format!("elapsed: {}ms", self.elapsed_ms),
        ]
        .join("\n")
    }
}

pub fn render_grouped_activity(activities: &[ToolActivity]) -> String {
    activities
        .iter()
        .map(ToolActivity::compact_line)
        .collect::<Vec<_>>()
        .join("\n")
}

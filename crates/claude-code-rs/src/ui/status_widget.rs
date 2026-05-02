//! Built-in rich status widget.

#[derive(Debug, Clone, PartialEq)]
pub struct StatusSnapshot {
    pub model: String,
    pub cwd: String,
    pub permission_mode: String,
    pub sandbox: String,
    pub cost_usd: f64,
    pub running_tools: usize,
    pub active_agents: usize,
    pub subsystems_ok: bool,
}

impl StatusSnapshot {
    pub fn render_line(&self) -> String {
        format!(
            "{} | {} | perm={} | sandbox={} | tools={} | agents={} | ${:.4}",
            self.model,
            self.cwd,
            self.permission_mode,
            self.sandbox,
            self.running_tools,
            self.active_agents,
            self.cost_usd
        )
    }

    pub fn render_details(&self) -> String {
        [
            format!("model: {}", self.model),
            format!("cwd: {}", self.cwd),
            format!("permission: {}", self.permission_mode),
            format!("sandbox: {}", self.sandbox),
            format!("running tools: {}", self.running_tools),
            format!("active agents: {}", self.active_agents),
            format!(
                "subsystems: {}",
                if self.subsystems_ok {
                    "ok"
                } else {
                    "attention"
                }
            ),
        ]
        .join("\n")
    }
}

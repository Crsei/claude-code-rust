//! Teams dialog rendering.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeammateStatus {
    pub name: String,
    pub role: String,
    pub mode: String,
    pub state: String,
    pub assigned_tasks: usize,
    pub hidden: bool,
}

impl TeammateStatus {
    pub fn new(name: impl Into<String>, role: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            role: role.into(),
            mode: "ask".to_string(),
            state: "running".to_string(),
            assigned_tasks: 0,
            hidden: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSummary {
    pub name: String,
    pub teammates: Vec<TeammateStatus>,
}

pub fn render_teams_dialog(team: &TeamSummary, selected_index: usize) -> String {
    let mut lines = vec![format!("Team: {}", team.name)];
    if team.teammates.is_empty() {
        lines.push("No teammates".to_string());
        return lines.join("\n");
    }

    for (idx, teammate) in team.teammates.iter().enumerate() {
        if teammate.name == "team-lead" {
            continue;
        }
        let marker = if idx == selected_index { ">" } else { " " };
        let hidden = if teammate.hidden { " hidden" } else { "" };
        lines.push(format!(
            "{marker} {:<14} {:<10} mode={} tasks={}{}",
            teammate.name, teammate.state, teammate.mode, teammate.assigned_tasks, hidden
        ));
        lines.push(format!("  role: {}", teammate.role));
    }
    lines.push("k kill | s shutdown | h hide | m cycle mode".to_string());
    lines.join("\n")
}

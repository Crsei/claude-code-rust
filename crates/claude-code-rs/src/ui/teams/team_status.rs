//! Footer status indicator for active teammates.

pub fn render_team_status(
    total_teammates: usize,
    teams_selected: bool,
    show_hint: bool,
) -> Option<String> {
    if total_teammates == 0 {
        return None;
    }
    let noun = if total_teammates == 1 {
        "teammate"
    } else {
        "teammates"
    };
    let mut status = format!("{total_teammates} {noun}");
    if teams_selected {
        status = format!("[{status}]");
    }
    if teams_selected && show_hint {
        status.push_str(" - Enter to view");
    }
    Some(status)
}

//! Shared render primitive for compact text tab bars.

pub fn render_tabs(labels: &[impl AsRef<str>], selected: usize) -> String {
    labels
        .iter()
        .enumerate()
        .map(|(idx, label)| render_tab(label.as_ref(), idx == selected))
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_tab(label: &str, selected: bool) -> String {
    if selected {
        format!("[{label}]")
    } else {
        format!(" {label} ")
    }
}

#[cfg(test)]
mod tests {
    use super::render_tabs;

    #[test]
    fn render_tabs_matches_command_surface_pattern() {
        assert_eq!(render_tabs(&["Status", "Config"], 1), " Status  [Config]");
    }
}

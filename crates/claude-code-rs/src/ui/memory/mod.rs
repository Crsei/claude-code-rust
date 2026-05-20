//! Rust-side memory UI surfaces.
pub mod memory_file_selector;
pub mod memory_update_notification;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::memory_file_selector::{MemoryFileKind, MemoryFileOption, MemoryFileSelectorState};
    use super::memory_update_notification::render_memory_update_notification;

    #[test]
    fn snapshot_memory_surfaces() {
        let cwd = Path::new("C:/repo/project");
        let home = Path::new("C:/Users/alice");
        let mut state = MemoryFileSelectorState::new(vec![
            MemoryFileOption::new("C:/Users/alice/.cc-rust/CLAUDE.md", MemoryFileKind::User),
            MemoryFileOption::new("C:/repo/project/CLAUDE.md", MemoryFileKind::Project).missing(),
            MemoryFileOption::new("C:/repo/project/docs/AGENTS.md", MemoryFileKind::Nested)
                .with_parent("C:/repo/project/CLAUDE.md"),
            MemoryFileOption::new("C:/Users/alice/.cc-rust/memory", MemoryFileKind::Folder)
                .with_description("auto-memory folder"),
        ]);
        state.selected_index = 1;

        let rendered = format!(
            "## selector\n{}\n\n## notification\n{}",
            state.render(cwd, home),
            render_memory_update_notification(Path::new("C:/repo/project/CLAUDE.md"), cwd, home)
        );

        insta::assert_snapshot!("memory_surfaces", rendered);
    }
}

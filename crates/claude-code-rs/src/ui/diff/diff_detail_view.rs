use super::{truncate_by_width, DiffFile};

const MAX_CONTEXT_LINES: usize = 400;
const LARGE_FILE_LIMIT_NOTE: &str = "Large file - diff exceeds 1 MB limit";
const BINARY_NOTE: &str = "Binary file - cannot display diff";
const UNTRACKED_NOTE: &str = "New file not yet staged.";
const UNTRACKED_HINT: &str = "Run `git add {file}` to see line counts.";
const NO_DIFF_NOTE: &str = "No diff content";
const TRUNCATED_NOTE: &str = "(diff truncated (exceeded 400 line limit))";

fn split_hunks(lines: &[String]) -> Vec<String> {
    let mut output = Vec::new();
    for chunk in lines {
        output.extend(chunk.split('\n').map(str::to_string));
    }
    output
}

fn render_plain_lines(lines: Vec<String>, width: usize) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| truncate_by_width(&line, width))
        .collect()
}

/// Render the detail view for a selected file.
pub fn render_diff_detail_view_lines(
    file: &DiffFile,
    hunks: &[String],
    width: usize,
) -> Vec<String> {
    if file.is_untracked {
        return vec![
            file.path.clone(),
            UNTRACKED_NOTE.to_string(),
            UNTRACKED_HINT.replace("{file}", &file.path),
        ];
    }

    if file.is_binary {
        return vec![file.path.clone(), BINARY_NOTE.to_string()];
    }

    if file.is_large_file {
        return vec![file.path.clone(), LARGE_FILE_LIMIT_NOTE.to_string()];
    }

    let mut lines = vec![file.path.clone()];
    let mut body = split_hunks(hunks);
    if body.is_empty() {
        body.push(NO_DIFF_NOTE.to_string());
    } else if body.len() > MAX_CONTEXT_LINES {
        body.truncate(MAX_CONTEXT_LINES);
    }

    lines.extend(render_plain_lines(body, width));
    if file.is_truncated {
        lines.push(TRUNCATED_NOTE.to_string());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{render_diff_detail_view_lines, DiffFile};
    use insta::assert_snapshot;

    #[test]
    fn snapshot_diff_detail_view_for_untracked() {
        let file = DiffFile::new("foo.rs", 0, 0, false, false, false, true);
        let output = render_diff_detail_view_lines(&file, &[], 80);
        assert_snapshot!(output.join("\n"));
    }

    #[test]
    fn snapshot_diff_detail_view_for_binary() {
        let file = DiffFile::new("asset.bin", 0, 0, true, false, false, false);
        let output = render_diff_detail_view_lines(&file, &[], 80);
        assert_snapshot!(output.join("\n"));
    }
}

use super::truncate_by_width;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredDiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub section: Option<String>,
    pub lines: Vec<StructuredDiffLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuredDiffLineKind {
    Context,
    Add,
    Remove,
    NoNewline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredDiffLine {
    pub kind: StructuredDiffLineKind,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub content: String,
}

impl StructuredDiffHunk {
    pub fn header(&self) -> String {
        let old_range = format_range(self.old_start, self.old_lines);
        let new_range = format_range(self.new_start, self.new_lines);
        match self
            .section
            .as_deref()
            .filter(|section| !section.is_empty())
        {
            Some(section) => format!("@@ -{old_range} +{new_range} @@ {section}"),
            None => format!("@@ -{old_range} +{new_range} @@"),
        }
    }
}

fn format_range(start: usize, lines: usize) -> String {
    if lines == 1 {
        start.to_string()
    } else {
        format!("{start},{lines}")
    }
}

/// Parse unified-diff hunk headers and their following body lines.
///
/// The input may include file headers (`diff --git`, `index`, `---`, `+++`) or
/// other patch metadata. Those lines are ignored until an `@@ ... @@` hunk
/// header is encountered.
pub fn parse_structured_hunks(lines: &[String]) -> Vec<StructuredDiffHunk> {
    let mut hunks = Vec::new();
    let mut current: Option<StructuredDiffHunk> = None;
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    for raw_line in lines {
        if let Some((old_start, old_lines, new_start, new_lines, section)) =
            parse_hunk_header(raw_line)
        {
            if let Some(hunk) = current.take() {
                hunks.push(hunk);
            }
            current = Some(StructuredDiffHunk {
                old_start,
                old_lines,
                new_start,
                new_lines,
                section,
                lines: Vec::new(),
            });
            old_line = old_start;
            new_line = new_start;
            continue;
        }

        let Some(hunk) = current.as_mut() else {
            continue;
        };

        if raw_line.starts_with("\\ No newline at end of file") {
            hunk.lines.push(StructuredDiffLine {
                kind: StructuredDiffLineKind::NoNewline,
                old_line: None,
                new_line: None,
                content: raw_line.clone(),
            });
            continue;
        }

        let Some((prefix, content)) = split_diff_line(raw_line) else {
            continue;
        };
        match prefix {
            ' ' => {
                hunk.lines.push(StructuredDiffLine {
                    kind: StructuredDiffLineKind::Context,
                    old_line: Some(old_line),
                    new_line: Some(new_line),
                    content: content.to_string(),
                });
                old_line += 1;
                new_line += 1;
            }
            '+' => {
                hunk.lines.push(StructuredDiffLine {
                    kind: StructuredDiffLineKind::Add,
                    old_line: None,
                    new_line: Some(new_line),
                    content: content.to_string(),
                });
                new_line += 1;
            }
            '-' => {
                hunk.lines.push(StructuredDiffLine {
                    kind: StructuredDiffLineKind::Remove,
                    old_line: Some(old_line),
                    new_line: None,
                    content: content.to_string(),
                });
                old_line += 1;
            }
            _ => {}
        }
    }

    if let Some(hunk) = current {
        hunks.push(hunk);
    }

    hunks
}

pub fn render_structured_diff_hunks(
    hunks: &[StructuredDiffHunk],
    width: usize,
    max_lines: usize,
) -> Vec<String> {
    if hunks.is_empty() || max_lines == 0 {
        return Vec::new();
    }

    let number_width = line_number_width(hunks);
    let mut rendered = Vec::new();
    for (hunk_index, hunk) in hunks.iter().enumerate() {
        if hunk_index > 0 {
            push_truncated(&mut rendered, "...", width, max_lines);
        }
        push_truncated(&mut rendered, &hunk.header(), width, max_lines);
        for line in &hunk.lines {
            let raw = render_structured_line(line, number_width);
            push_truncated(&mut rendered, &raw, width, max_lines);
        }
        if rendered.len() >= max_lines {
            break;
        }
    }
    rendered
}

fn push_truncated(lines: &mut Vec<String>, raw: &str, width: usize, max_lines: usize) {
    if lines.len() >= max_lines {
        return;
    }
    lines.push(truncate_by_width(raw, width));
}

fn render_structured_line(line: &StructuredDiffLine, number_width: usize) -> String {
    if line.kind == StructuredDiffLineKind::NoNewline {
        let gutter_width = number_width * 2 + 4;
        return format!("{:gutter_width$}{}", "", line.content);
    }

    let old = format_optional_line(line.old_line, number_width);
    let new = format_optional_line(line.new_line, number_width);
    let sigil = match line.kind {
        StructuredDiffLineKind::Context => ' ',
        StructuredDiffLineKind::Add => '+',
        StructuredDiffLineKind::Remove => '-',
        StructuredDiffLineKind::NoNewline => ' ',
    };
    format!("{old} {new} {sigil} {}", line.content)
}

fn format_optional_line(line: Option<usize>, width: usize) -> String {
    match line {
        Some(line) => format!("{line:>width$}"),
        None => " ".repeat(width),
    }
}

fn line_number_width(hunks: &[StructuredDiffHunk]) -> usize {
    let max_line = hunks
        .iter()
        .flat_map(|hunk| {
            [
                hunk.old_start
                    .saturating_add(hunk.old_lines.saturating_sub(1)),
                hunk.new_start
                    .saturating_add(hunk.new_lines.saturating_sub(1)),
            ]
        })
        .max()
        .unwrap_or(1);
    max_line.to_string().len().max(4)
}

fn split_diff_line(line: &str) -> Option<(char, &str)> {
    let mut chars = line.chars();
    let prefix = chars.next()?;
    if matches!(prefix, ' ' | '+' | '-') {
        Some((prefix, chars.as_str()))
    } else {
        None
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize, Option<String>)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old_range, rest) = rest.split_once(" +")?;
    let (new_range, rest) = rest.split_once(" @@")?;
    let (old_start, old_lines) = parse_range(old_range)?;
    let (new_start, new_lines) = parse_range(new_range)?;
    let section = rest.trim_start().trim().to_string();
    let section = if section.is_empty() {
        None
    } else {
        Some(section)
    };
    Some((old_start, old_lines, new_start, new_lines, section))
}

fn parse_range(raw: &str) -> Option<(usize, usize)> {
    if let Some((start, lines)) = raw.split_once(',') {
        Some((start.parse().ok()?, lines.parse().ok()?))
    } else {
        Some((raw.parse().ok()?, 1))
    }
}

#[cfg(test)]
mod tests {
    use super::{StructuredDiffLineKind, parse_structured_hunks, render_structured_diff_hunks};
    use insta::assert_snapshot;

    #[test]
    fn parses_hunk_header_ranges_and_lines() {
        let input = vec![
            "diff --git a/src/lib.rs b/src/lib.rs".to_string(),
            "@@ -10,2 +10,3 @@ fn demo()".to_string(),
            " context".to_string(),
            "-old".to_string(),
            "+new".to_string(),
            "+extra".to_string(),
            "\\ No newline at end of file".to_string(),
        ];

        let hunks = parse_structured_hunks(&input);

        assert_eq!(hunks.len(), 1);
        assert_eq!(hunks[0].old_start, 10);
        assert_eq!(hunks[0].old_lines, 2);
        assert_eq!(hunks[0].new_start, 10);
        assert_eq!(hunks[0].new_lines, 3);
        assert_eq!(hunks[0].section.as_deref(), Some("fn demo()"));
        assert_eq!(hunks[0].lines[0].old_line, Some(10));
        assert_eq!(hunks[0].lines[0].new_line, Some(10));
        assert_eq!(hunks[0].lines[1].kind, StructuredDiffLineKind::Remove);
        assert_eq!(hunks[0].lines[1].old_line, Some(11));
        assert_eq!(hunks[0].lines[2].kind, StructuredDiffLineKind::Add);
        assert_eq!(hunks[0].lines[2].new_line, Some(11));
        assert_eq!(hunks[0].lines[3].new_line, Some(12));
        assert_eq!(hunks[0].lines[4].kind, StructuredDiffLineKind::NoNewline);
    }

    #[test]
    fn parses_single_line_ranges_and_multiple_hunks() {
        let input = vec![
            "@@ -1 +1 @@".to_string(),
            "-old".to_string(),
            "+new".to_string(),
            "@@ -20,0 +21,2 @@ inserted".to_string(),
            "+alpha".to_string(),
            "+beta".to_string(),
        ];

        let hunks = parse_structured_hunks(&input);

        assert_eq!(hunks.len(), 2);
        assert_eq!(hunks[0].old_lines, 1);
        assert_eq!(hunks[0].new_lines, 1);
        assert_eq!(hunks[1].old_start, 20);
        assert_eq!(hunks[1].old_lines, 0);
        assert_eq!(hunks[1].new_start, 21);
        assert_eq!(hunks[1].new_lines, 2);
    }

    #[test]
    fn ignores_raw_lines_without_hunk_headers() {
        let input = vec!["+legacy line".to_string(), "-legacy old".to_string()];

        assert!(parse_structured_hunks(&input).is_empty());
    }

    #[test]
    fn snapshot_renders_structured_hunks_with_old_and_new_gutters() {
        let input = vec![
            "@@ -1,3 +1,4 @@ fn main".to_string(),
            " fn main() {".to_string(),
            "-    old_call();".to_string(),
            "+    new_call();".to_string(),
            "+    extra_call();".to_string(),
            " }".to_string(),
            "@@ -20,1 +21,1 @@".to_string(),
            "-let stale = true;".to_string(),
            "+let stale = false;".to_string(),
            "\\ No newline at end of file".to_string(),
        ];
        let hunks = parse_structured_hunks(&input);

        assert_snapshot!(render_structured_diff_hunks(&hunks, 80, 100).join("\n"));
    }
}

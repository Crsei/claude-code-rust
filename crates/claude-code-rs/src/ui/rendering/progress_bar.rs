//! Shared terminal progress-bar rendering.

use ratatui::text::{Line, Span};

use super::theme::Theme;

const BLOCKS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

pub fn render_progress_bar(ratio: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let ratio = if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let scaled = ratio * width as f64;
    let whole = scaled.floor() as usize;
    let filled = BLOCKS[BLOCKS.len() - 1].repeat(whole.min(width));

    if whole >= width {
        return filled;
    }

    let remainder = scaled - whole as f64;
    let partial = ((remainder * BLOCKS.len() as f64).floor() as usize).min(BLOCKS.len() - 1);
    let empty = width - whole - 1;
    format!("{}{}{}", filled, BLOCKS[partial], BLOCKS[0].repeat(empty))
}

/// Render a progress bar with theme-aware colors.
///
/// The filled portion uses `theme.progress_fill` and the empty portion uses
/// `theme.progress_empty`.
pub fn render_styled_progress_bar(ratio: f64, width: usize, theme: &Theme) -> Line<'static> {
    if width == 0 {
        return Line::default();
    }

    let ratio = if ratio.is_finite() {
        ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let scaled = ratio * width as f64;
    let whole = scaled.floor() as usize;

    let full_char = BLOCKS[BLOCKS.len() - 1];
    let filled_count = whole.min(width);

    if whole >= width {
        return Line::from(Span::styled(full_char.repeat(width), theme.progress_fill));
    }

    let remainder = scaled - whole as f64;
    let partial_idx = ((remainder * BLOCKS.len() as f64).floor() as usize).min(BLOCKS.len() - 1);
    let empty_count = width - filled_count - 1;

    let mut spans = Vec::with_capacity(3);
    if filled_count > 0 {
        spans.push(Span::styled(
            full_char.repeat(filled_count),
            theme.progress_fill,
        ));
    }
    spans.push(Span::styled(
        BLOCKS[partial_idx].to_string(),
        theme.progress_fill,
    ));
    if empty_count > 0 {
        spans.push(Span::styled(
            BLOCKS[0].repeat(empty_count),
            theme.progress_empty,
        ));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::render_progress_bar;

    #[test]
    fn clamps_out_of_range_ratios() {
        assert_eq!(render_progress_bar(-1.0, 4), "    ");
        assert_eq!(render_progress_bar(2.0, 4), "████");
        assert_eq!(render_progress_bar(f64::NAN, 4), "    ");
    }

    #[test]
    fn renders_empty_partial_and_full_bars() {
        assert_eq!(render_progress_bar(0.0, 4), "    ");
        assert_eq!(render_progress_bar(0.125, 4), "▌   ");
        assert_eq!(render_progress_bar(0.25, 4), "█   ");
        assert_eq!(render_progress_bar(0.5, 4), "██  ");
        assert_eq!(render_progress_bar(1.0, 4), "████");
    }

    #[test]
    fn supports_zero_width() {
        assert_eq!(render_progress_bar(0.5, 0), "");
    }

    #[test]
    fn styled_bar_uses_theme_colors() {
        let theme = super::Theme::default();
        let styled = super::render_styled_progress_bar(0.5, 4, &theme);
        assert_eq!(styled.spans.len(), 3); // filled + partial + empty
    }
}

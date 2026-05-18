//! Shared terminal progress-bar rendering.
//!
//! Provides the low-level [`render_progress_bar`] string function and an
//! enhanced [`ProgressBar`] widget with theme-aware colours.

#![allow(dead_code)]

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::ui::theme::ThemeColors;
use crate::ui::theme::color::resolve_color;

const BLOCKS: [&str; 9] = [" ", "▏", "▎", "▍", "▌", "▋", "▊", "▉", "█"];

/// Low-level progress bar renderer returning a plain string.
///
/// Uses Unicode ⅛-block characters for smooth rendering.
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

/// Theme-aware progress bar widget.
///
/// Wraps [`render_progress_bar`] with fill/empty colours from the theme.
pub struct ProgressBar {
    /// Progress ratio (0.0 – 1.0).
    pub ratio: f64,
    /// Width in columns.
    pub width: usize,
    /// Optional theme colour key for filled portion (default: `accent`).
    pub fill_color: Option<&'static str>,
    /// Optional theme colour key for empty portion (default: `inactive`).
    pub empty_color: Option<&'static str>,
}

impl ProgressBar {
    pub fn new(ratio: f64, width: usize) -> Self {
        Self {
            ratio,
            width,
            fill_color: None,
            empty_color: None,
        }
    }

    /// Render the progress bar as styled spans, keeping filled and empty
    /// portions visually distinct.
    pub fn render(&self, colors: &ThemeColors) -> Line<'static> {
        let fill_color = self
            .fill_color
            .and_then(|key| resolve_color(key, colors))
            .unwrap_or(colors.accent);
        let empty_color = self
            .empty_color
            .and_then(|key| resolve_color(key, colors))
            .unwrap_or(colors.inactive);

        let ratio = self.ratio.clamp(0.0, 1.0);
        let total_eighths = (ratio * (self.width as f64) * 8.0).round() as usize;
        let full_blocks = (total_eighths / 8).min(self.width);
        let partial = if full_blocks < self.width {
            total_eighths % 8
        } else {
            0
        };
        let empty_blocks = self
            .width
            .saturating_sub(full_blocks)
            .saturating_sub(usize::from(partial > 0));

        let mut spans = Vec::new();
        if full_blocks > 0 {
            spans.push(Span::styled(
                BLOCKS[8].repeat(full_blocks),
                Style::default().fg(fill_color),
            ));
        }
        if partial > 0 {
            spans.push(Span::styled(
                BLOCKS[partial].to_string(),
                Style::default().fg(fill_color),
            ));
        }
        if empty_blocks > 0 {
            spans.push(Span::styled(
                BLOCKS[0].repeat(empty_blocks),
                Style::default().fg(empty_color),
            ));
        }

        Line::from(spans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::{ThemeName, get_theme};

    fn dark() -> &'static ThemeColors {
        get_theme(&ThemeName::Dark)
    }

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
    fn progress_bar_widget_uses_accent() {
        let pb = ProgressBar::new(0.5, 10);
        let line = pb.render(dark());
        assert_eq!(line.spans[0].style.fg, Some(dark().accent));
        assert_eq!(line.spans.last().unwrap().style.fg, Some(dark().inactive));
    }

    #[test]
    fn progress_bar_widget_respects_custom_colours() {
        let mut pb = ProgressBar::new(0.25, 8);
        pb.fill_color = Some("success");
        pb.empty_color = Some("warning");
        let line = pb.render(dark());
        assert_eq!(line.spans[0].style.fg, Some(dark().success));
        assert_eq!(line.spans.last().unwrap().style.fg, Some(dark().warning));
    }
}

//! Shimmer text styling for loading labels.

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use ratatui::style::{Color, Style};
use ratatui::text::Span;

static START: OnceLock<Instant> = OnceLock::new();

fn elapsed_since_start() -> Duration {
    START.get_or_init(Instant::now).elapsed()
}

pub(crate) fn shimmer_spans(text: &str) -> Vec<Span<'static>> {
    let elapsed = elapsed_since_start().as_millis() as f32 / 160.0;
    text.chars()
        .enumerate()
        .map(|(idx, ch)| {
            let wave = ((idx as f32 * 0.55 + elapsed).sin() + 1.0) * 0.5;
            Span::styled(ch.to_string(), color_for_level(wave))
        })
        .collect()
}

fn color_for_level(intensity: f32) -> Style {
    let clamped = intensity.clamp(0.0, 1.0);
    let base = 120.0 + clamped * 100.0;
    Style::default().fg(Color::Rgb(base as u8, base as u8, base as u8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_one_span_per_char() {
        assert_eq!(shimmer_spans("abc").len(), 3);
    }
}

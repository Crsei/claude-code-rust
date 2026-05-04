//! Shared terminal progress-bar rendering.

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
}

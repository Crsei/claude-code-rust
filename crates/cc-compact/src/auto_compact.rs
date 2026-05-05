#![allow(unused)]

pub use cc_utils::tokens::get_context_window_size;

/// Check if auto-compaction should be triggered based on token count.
///
/// Returns true when the estimated token usage exceeds 80% of the
/// model's context window, indicating that a compaction pass should
/// be run to free up space.
pub fn should_auto_compact(estimated_tokens: u64, model: &str) -> bool {
    let context_window = get_context_window_size(model);
    let threshold = (context_window as f64 * 0.8) as u64;
    estimated_tokens > threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_auto_compact_below_threshold() {
        // 80% of 200k = 160k
        assert!(!should_auto_compact(100_000, "claude-sonnet-4-20250514"));
        assert!(!should_auto_compact(159_999, "claude-sonnet-4-20250514"));
    }

    #[test]
    fn test_should_auto_compact_above_threshold() {
        assert!(should_auto_compact(160_001, "claude-sonnet-4-20250514"));
        assert!(should_auto_compact(200_000, "claude-opus-4-20250514"));
    }

    #[test]
    fn test_should_auto_compact_respects_1m_suffix() {
        assert!(!should_auto_compact(
            200_000,
            "claude-sonnet-4-20250514[1m]"
        ));
        assert!(should_auto_compact(800_001, "claude-sonnet-4-20250514[1m]"));
    }

    #[test]
    fn test_context_window_sizes() {
        assert_eq!(get_context_window_size("claude-opus-4-20250514"), 200_000);
        assert_eq!(get_context_window_size("claude-sonnet-4-20250514"), 200_000);
        assert_eq!(
            get_context_window_size("claude-haiku-3-5-20241022"),
            200_000
        );
        assert_eq!(
            get_context_window_size("claude-sonnet-4-20250514[1m]"),
            1_000_000
        );
        assert_eq!(get_context_window_size("unknown-model"), 200_000);
    }
}

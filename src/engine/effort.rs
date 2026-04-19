//! Effort level → thinking budget tokens mapping.
//!
//! Used by `build_messages_request` to translate a user-facing effort
//! label (`low` / `medium` / `high`) or numeric override into the
//! `thinking.budget_tokens` value sent to the model API.
//!
//! Per issue #9 scope: this is the *only* effort behavior wired up.
//! `/effort auto`, `/effort max`, and model-compatibility checks are
//! intentionally not implemented.

/// Default budget when thinking is enabled but no effort level is set.
pub const DEFAULT_THINKING_BUDGET: u32 = 10_240;

/// Map an effort label to a `thinking.budget_tokens` value.
///
/// Accepts:
///   - `"low"` / `"medium"` / `"high"` (case-insensitive)
///   - A numeric string (e.g. `"8000"`) used directly as the budget
///
/// Returns `None` for empty input or unrecognized non-numeric labels.
pub fn effort_to_budget_tokens(effort: &str) -> Option<u32> {
    let trimmed = effort.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(n) = trimmed.parse::<u32>() {
        if n == 0 {
            return None;
        }
        return Some(n);
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "low" => Some(4_096),
        "medium" | "med" => Some(10_240),
        "high" => Some(24_576),
        _ => None,
    }
}

/// Resolve the effective thinking budget for a request.
///
/// Returns the budget in priority order:
///   1. `effort_to_budget_tokens(effort_value)` if it parses
///   2. `max_output_tokens_fallback` if provided
///   3. [`DEFAULT_THINKING_BUDGET`]
pub fn resolve_thinking_budget(
    effort_value: Option<&str>,
    max_output_tokens_fallback: Option<u32>,
) -> u32 {
    effort_value
        .and_then(effort_to_budget_tokens)
        .or(max_output_tokens_fallback)
        .unwrap_or(DEFAULT_THINKING_BUDGET)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_labels_map_to_budgets() {
        assert_eq!(effort_to_budget_tokens("low"), Some(4_096));
        assert_eq!(effort_to_budget_tokens("medium"), Some(10_240));
        assert_eq!(effort_to_budget_tokens("high"), Some(24_576));
    }

    #[test]
    fn labels_are_case_insensitive() {
        assert_eq!(effort_to_budget_tokens("LOW"), Some(4_096));
        assert_eq!(effort_to_budget_tokens("Medium"), Some(10_240));
        assert_eq!(effort_to_budget_tokens("HIGH"), Some(24_576));
    }

    #[test]
    fn numeric_override_returned_directly() {
        assert_eq!(effort_to_budget_tokens("4096"), Some(4_096));
        assert_eq!(effort_to_budget_tokens("32000"), Some(32_000));
        assert_eq!(effort_to_budget_tokens(" 8000 "), Some(8_000));
    }

    #[test]
    fn zero_and_empty_return_none() {
        assert_eq!(effort_to_budget_tokens(""), None);
        assert_eq!(effort_to_budget_tokens("   "), None);
        assert_eq!(effort_to_budget_tokens("0"), None);
    }

    #[test]
    fn unknown_labels_return_none() {
        assert_eq!(effort_to_budget_tokens("ultra"), None);
        assert_eq!(effort_to_budget_tokens("auto"), None);
        assert_eq!(effort_to_budget_tokens("max"), None);
        assert_eq!(effort_to_budget_tokens("not-a-number-or-label"), None);
    }

    #[test]
    fn resolve_prefers_effort_label() {
        assert_eq!(resolve_thinking_budget(Some("high"), Some(99_999)), 24_576);
    }

    #[test]
    fn resolve_falls_back_to_max_tokens() {
        assert_eq!(resolve_thinking_budget(None, Some(8_000)), 8_000);
        assert_eq!(resolve_thinking_budget(Some("ultra"), Some(8_000)), 8_000);
    }

    #[test]
    fn resolve_falls_back_to_default() {
        assert_eq!(resolve_thinking_budget(None, None), DEFAULT_THINKING_BUDGET);
        assert_eq!(
            resolve_thinking_budget(Some(""), None),
            DEFAULT_THINKING_BUDGET
        );
    }
}

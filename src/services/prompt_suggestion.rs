//! Prompt suggestion service — generates next-prompt suggestions based on
//! conversation context using local heuristics (no API call).
//!
//! Suggestions are rate-limited to avoid excessive generation.

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single prompt suggestion offered to the user.
#[derive(Debug, Clone)]
pub struct PromptSuggestion {
    /// Suggested prompt text.
    pub text: String,
    /// Confidence score from 0.0 (low) to 1.0 (high).
    pub confidence: f32,
    /// Category of the suggestion.
    pub category: SuggestionCategory,
}

/// Categorization of a prompt suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuggestionCategory {
    /// Natural follow-up to the current topic.
    FollowUp,
    /// Alternative approach to the problem.
    Alternative,
    /// Request for clarification.
    Clarification,
    /// Suggest a concrete action.
    Action,
}

/// Reason why prompt suggestions are suppressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuppressionReason {
    /// The service is disabled.
    Disabled,
    /// Plan mode is currently active.
    PlanModeActive,
    /// Not enough messages in the conversation yet.
    TooFewMessages,
    /// Rate-limited — too soon since last generation.
    RateLimited,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

/// Generates prompt suggestions based on conversation context.
pub struct PromptSuggestionService {
    enabled: bool,
    last_generation_time: Option<Instant>,
    min_interval: Duration,
}

impl PromptSuggestionService {
    /// Create a new prompt suggestion service.
    pub fn new(enabled: bool) -> Self {
        PromptSuggestionService {
            enabled,
            last_generation_time: None,
            min_interval: Duration::from_secs(30),
        }
    }

    /// Check if the service is enabled.
    pub fn should_enable(&self) -> bool {
        self.enabled
    }

    /// Determine if suggestions should be suppressed, and if so, why.
    pub fn get_suppression_reason(
        &self,
        message_count: usize,
        plan_mode: bool,
    ) -> Option<SuppressionReason> {
        if !self.enabled {
            return Some(SuppressionReason::Disabled);
        }
        if plan_mode {
            return Some(SuppressionReason::PlanModeActive);
        }
        if message_count < 2 {
            return Some(SuppressionReason::TooFewMessages);
        }
        if let Some(last) = self.last_generation_time {
            if last.elapsed() < self.min_interval {
                return Some(SuppressionReason::RateLimited);
            }
        }
        None
    }

    /// Try to generate prompt suggestions based on conversation context.
    ///
    /// Uses local heuristics based on which tools were used. Returns `None`
    /// if generation is suppressed (disabled, rate-limited, etc.).
    pub fn try_generate(
        &mut self,
        messages_summary: &str,
        tool_names: &[String],
    ) -> Option<Vec<PromptSuggestion>> {
        if !self.enabled {
            return None;
        }

        // Rate limiting
        if let Some(last) = self.last_generation_time {
            if last.elapsed() < self.min_interval {
                return None;
            }
        }

        let mut suggestions = Vec::new();

        // Heuristic: tool-based suggestions
        for name in tool_names {
            match name.as_str() {
                "Bash" => {
                    suggestions.push(PromptSuggestion {
                        text: "Run the tests to verify the changes".to_string(),
                        confidence: 0.7,
                        category: SuggestionCategory::Action,
                    });
                }
                "Edit" | "FileEdit" => {
                    suggestions.push(PromptSuggestion {
                        text: "Review the changes I just made".to_string(),
                        confidence: 0.6,
                        category: SuggestionCategory::FollowUp,
                    });
                }
                "Write" | "FileWrite" => {
                    suggestions.push(PromptSuggestion {
                        text: "Check the file was written correctly".to_string(),
                        confidence: 0.5,
                        category: SuggestionCategory::FollowUp,
                    });
                }
                "Grep" | "Glob" => {
                    suggestions.push(PromptSuggestion {
                        text: "Search for related patterns in other files".to_string(),
                        confidence: 0.5,
                        category: SuggestionCategory::Alternative,
                    });
                }
                "Read" | "FileRead" => {
                    suggestions.push(PromptSuggestion {
                        text: "Explain what this code does".to_string(),
                        confidence: 0.6,
                        category: SuggestionCategory::FollowUp,
                    });
                }
                _ => {}
            }
        }

        // Heuristic: if messages mention "error" or "bug", suggest debugging
        let summary_lower = messages_summary.to_lowercase();
        if summary_lower.contains("error") || summary_lower.contains("bug") {
            suggestions.push(PromptSuggestion {
                text: "Try a different approach to fix this".to_string(),
                confidence: 0.6,
                category: SuggestionCategory::Alternative,
            });
        }

        // Heuristic: if messages mention "test", suggest running tests
        if summary_lower.contains("test") {
            suggestions.push(PromptSuggestion {
                text: "Run the test suite".to_string(),
                confidence: 0.7,
                category: SuggestionCategory::Action,
            });
        }

        // Always offer a clarification option if we have any suggestions
        if !suggestions.is_empty() {
            suggestions.push(PromptSuggestion {
                text: "Can you explain your approach?".to_string(),
                confidence: 0.3,
                category: SuggestionCategory::Clarification,
            });
        }

        self.last_generation_time = Some(Instant::now());

        if suggestions.is_empty() {
            None
        } else {
            // Sort by confidence descending
            suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
            Some(suggestions)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_service_suppresses() {
        let svc = PromptSuggestionService::new(false);
        assert_eq!(
            svc.get_suppression_reason(10, false),
            Some(SuppressionReason::Disabled)
        );
    }

    #[test]
    fn plan_mode_suppresses() {
        let svc = PromptSuggestionService::new(true);
        assert_eq!(
            svc.get_suppression_reason(10, true),
            Some(SuppressionReason::PlanModeActive)
        );
    }

    #[test]
    fn too_few_messages_suppresses() {
        let svc = PromptSuggestionService::new(true);
        assert_eq!(
            svc.get_suppression_reason(1, false),
            Some(SuppressionReason::TooFewMessages)
        );
        assert_eq!(
            svc.get_suppression_reason(0, false),
            Some(SuppressionReason::TooFewMessages)
        );
    }

    #[test]
    fn no_suppression_when_conditions_met() {
        let svc = PromptSuggestionService::new(true);
        assert!(svc.get_suppression_reason(5, false).is_none());
    }

    #[test]
    fn generate_bash_suggestions() {
        let mut svc = PromptSuggestionService::new(true);
        let tools = vec!["Bash".to_string()];
        let result = svc.try_generate("running commands", &tools);
        assert!(result.is_some());
        let suggestions = result.unwrap();
        assert!(suggestions.iter().any(|s| s.text.contains("tests")));
    }

    #[test]
    fn generate_edit_suggestions() {
        let mut svc = PromptSuggestionService::new(true);
        let tools = vec!["Edit".to_string()];
        let result = svc.try_generate("editing files", &tools);
        assert!(result.is_some());
        let suggestions = result.unwrap();
        assert!(suggestions
            .iter()
            .any(|s| s.category == SuggestionCategory::FollowUp));
    }

    #[test]
    fn rate_limiting_blocks_second_call() {
        let mut svc = PromptSuggestionService::new(true);
        let tools = vec!["Bash".to_string()];

        let first = svc.try_generate("test", &tools);
        assert!(first.is_some());

        // Second call should be rate-limited
        let second = svc.try_generate("test", &tools);
        assert!(second.is_none());
    }

    #[test]
    fn disabled_generate_returns_none() {
        let mut svc = PromptSuggestionService::new(false);
        let result = svc.try_generate("anything", &["Bash".to_string()]);
        assert!(result.is_none());
    }

    #[test]
    fn empty_tools_and_no_keywords_returns_none() {
        let mut svc = PromptSuggestionService::new(true);
        let result = svc.try_generate("hello world", &[]);
        assert!(result.is_none());
    }
}

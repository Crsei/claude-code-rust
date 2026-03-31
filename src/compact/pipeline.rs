//! Context management pipeline — orchestrates all compaction steps.
//!
//! Corresponds to: LIFECYCLE_STATE_MACHINE.md §8 (Phase G)
//!
//! Pipeline order (each query loop iteration):
//!   1. applyToolResultBudget — persist oversized tool results to disk
//!   2. snipCompact — trim old turns beyond max_turns limit
//!   3. microcompact — remove cached/redundant tool results
//!   4. contextCollapse — fold old segments into summaries (Phase 2+)
//!   5. autoCompact — full summarization when nearing token limit
//!
//! Recovery paths (after API errors):
//!   - prompt_too_long (413):
//!     1. contextCollapse.recoverFromOverflow() → collapse_drain_retry
//!     2. reactiveCompact.tryReactiveCompact() → reactive_compact_retry
//!     3. Unrecoverable → return prompt_too_long
//!
//!   - max_output_tokens:
//!     1. Escalate 8k → 64k → max_output_tokens_escalate
//!     2. Recovery message injection (max 3) → max_output_tokens_recovery
//!     3. Exhausted → yield error, return completed

#![allow(unused)]

use anyhow::Result;
use tracing::{debug, info, warn};

use crate::types::message::Message;
use crate::types::state::AutoCompactTracking;
use crate::utils::tokens;

use super::auto_compact;
use super::microcompact;
use super::snip;
use super::tool_result_budget;

// ---------------------------------------------------------------------------
// Pipeline result types
// ---------------------------------------------------------------------------

/// Result of running the context management pipeline.
#[derive(Debug)]
pub struct PipelineResult {
    /// Processed messages (may be shorter than input).
    pub messages: Vec<Message>,
    /// Updated auto-compact tracking state.
    pub tracking: Option<AutoCompactTracking>,
    /// Whether any compaction was actually performed.
    pub compacted: bool,
    /// Estimated tokens after compaction.
    pub estimated_tokens: u64,
}

/// Result of a reactive compaction attempt.
#[derive(Debug, Clone)]
pub struct ReactiveCompactResult {
    pub messages: Vec<Message>,
    pub tracking: AutoCompactTracking,
    pub tokens_freed: u64,
}

/// Default maximum turns before snipping kicks in.
const DEFAULT_SNIP_MAX_TURNS: usize = 200;

/// Emergency snip target for reactive compaction.
const REACTIVE_SNIP_MAX_TURNS: usize = 5;

// ---------------------------------------------------------------------------
// Main pipeline (async — includes tool result budget disk I/O)
// ---------------------------------------------------------------------------

/// Run the full context management pipeline (steps 1-5).
///
/// # Arguments
/// * `messages` - Current conversation history
/// * `tracking` - Auto-compact tracking state from previous iteration
/// * `model` - Model name (for context window size lookup)
pub async fn run_context_pipeline(
    messages: Vec<Message>,
    tracking: Option<AutoCompactTracking>,
    model: &str,
) -> PipelineResult {
    let mut current = messages;
    let mut compacted = false;

    // ── Step 1: Tool result budget (async — saves oversized results to disk) ──
    let mut replacement_state = tool_result_budget::ContentReplacementState::default();
    let budgeted = tool_result_budget::apply_tool_result_budget(
        current,
        &mut replacement_state,
        100_000,
    )
    .await;
    if !replacement_state.replacements.is_empty() {
        compacted = true;
        debug!(
            replacements = replacement_state.replacements.len(),
            "tool result budget: persisted oversized results"
        );
    }
    current = budgeted;

    // ── Step 2: Snip compact ────────────────────────────────────────
    let snip_result = snip::snip_compact_if_needed(current, DEFAULT_SNIP_MAX_TURNS);
    if snip_result.tokens_freed > 0 {
        compacted = true;
        debug!(
            freed = snip_result.tokens_freed,
            "snip compact: trimmed old turns"
        );
    }
    current = snip_result.messages;

    // ── Step 3: Microcompact ────────────────────────────────────────
    let micro_result = microcompact::microcompact_messages(current);
    if micro_result.tokens_freed > 0 {
        compacted = true;
        debug!(
            freed = micro_result.tokens_freed,
            "microcompact: trimmed old tool results"
        );
    }
    current = micro_result.messages;

    // ── Step 4: Context collapse (Phase 2+) ─────────────────────────
    // Not yet implemented — will fold old segments into summaries.

    // ── Step 5: Auto compact check ──────────────────────────────────
    let estimated = tokens::estimate_messages_tokens(&current);
    let updated_tracking = if auto_compact::should_auto_compact(estimated, model) {
        info!(
            estimated_tokens = estimated,
            model = model,
            "auto compact triggered (>80% of context window)"
        );
        let base = tracking.unwrap_or(AutoCompactTracking {
            compacted: false,
            turn_counter: 0,
            turn_id: String::new(),
            consecutive_failures: 0,
        });
        Some(AutoCompactTracking {
            compacted: true,
            turn_counter: base.turn_counter + 1,
            turn_id: base.turn_id,
            consecutive_failures: base.consecutive_failures,
        })
    } else {
        tracking
    };

    PipelineResult {
        messages: current,
        tracking: updated_tracking,
        compacted,
        estimated_tokens: estimated,
    }
}

/// Attempt reactive compaction after a prompt_too_long error.
///
/// This is more aggressive than the normal pipeline — it aggressively
/// budgets tool results, snips to a very small number of turns, and
/// microcompacts.
///
/// Returns `None` if compaction is not possible (already small enough).
pub async fn try_reactive_compact(
    messages: Vec<Message>,
    model: &str,
) -> Option<ReactiveCompactResult> {
    let initial_tokens = tokens::estimate_messages_tokens(&messages);
    let target = (auto_compact::get_context_window_size(model) as f64 * 0.6) as u64;

    if initial_tokens <= target {
        return None; // Already within limits
    }

    // First: budget oversized tool results
    let mut replacement_state = tool_result_budget::ContentReplacementState::default();
    let current = tool_result_budget::apply_tool_result_budget(
        messages,
        &mut replacement_state,
        100_000,
    )
    .await;

    if !replacement_state.replacements.is_empty() {
        debug!(
            replacements = replacement_state.replacements.len(),
            "reactive compact: budgeted oversized tool results"
        );
    }

    // Aggressive strategy: snip to keep only the last few turns
    let snip_result = snip::snip_compact_if_needed(current, REACTIVE_SNIP_MAX_TURNS);
    if snip_result.tokens_freed == 0 && replacement_state.replacements.is_empty() {
        // Snipping didn't help — try microcompact alone
        let micro = microcompact::microcompact_messages(snip_result.messages);
        if micro.tokens_freed == 0 {
            return None;
        }
        let final_tokens = tokens::estimate_messages_tokens(&micro.messages);
        return Some(ReactiveCompactResult {
            messages: micro.messages,
            tracking: AutoCompactTracking {
                compacted: true,
                turn_counter: 0,
                turn_id: String::new(),
                consecutive_failures: 0,
            },
            tokens_freed: initial_tokens.saturating_sub(final_tokens),
        });
    }

    // Then microcompact the result
    let micro_result = microcompact::microcompact_messages(snip_result.messages);

    let final_tokens = tokens::estimate_messages_tokens(&micro_result.messages);
    let tokens_freed = initial_tokens.saturating_sub(final_tokens);

    if tokens_freed == 0 {
        return None;
    }

    Some(ReactiveCompactResult {
        messages: micro_result.messages,
        tracking: AutoCompactTracking {
            compacted: true,
            turn_counter: 0,
            turn_id: String::new(),
            consecutive_failures: 0,
        },
        tokens_freed,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::{
        ContentBlock, MessageContent, UserMessage, AssistantMessage,
    };
    use uuid::Uuid;

    fn make_user(text: &str) -> Message {
        Message::User(UserMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "user".into(),
            content: MessageContent::Text(text.into()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })
    }

    fn make_assistant(text: &str) -> Message {
        Message::Assistant(AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".into(),
            content: vec![ContentBlock::Text { text: text.into() }],
            usage: None,
            stop_reason: Some("end_turn".into()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        })
    }

    #[tokio::test]
    async fn test_pipeline_no_changes_small_conversation() {
        let messages = vec![make_user("Hello"), make_assistant("Hi!")];
        let result = run_context_pipeline(
            messages,
            None,
            "claude-sonnet-4-20250514",
        )
        .await;
        assert_eq!(result.messages.len(), 2);
    }

    #[tokio::test]
    async fn test_pipeline_returns_estimated_tokens() {
        let messages = vec![make_user("Hello"), make_assistant("Hi!")];
        let result = run_context_pipeline(messages, None, "claude-sonnet-4-20250514").await;
        assert!(result.estimated_tokens > 0);
    }

    #[tokio::test]
    async fn test_reactive_compact_small_conversation_returns_none() {
        let messages = vec![make_user("Hello"), make_assistant("Hi!")];
        let result = try_reactive_compact(messages, "claude-sonnet-4-20250514").await;
        assert!(result.is_none());
    }
}

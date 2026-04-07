//! Core types used across the QueryEngine lifecycle.

use crate::types::message::Usage;

/// Usage tracking -- accumulated across all API calls in a session.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct UsageTracking {
    /// Total input tokens consumed.
    pub total_input_tokens: u64,
    /// Total output tokens produced.
    pub total_output_tokens: u64,
    /// Total cache-read tokens.
    pub total_cache_read_tokens: u64,
    /// Total cache-creation tokens.
    pub total_cache_creation_tokens: u64,
    /// Total cost in USD.
    pub total_cost_usd: f64,
    /// Number of API calls made.
    pub api_call_count: u64,
}

impl UsageTracking {
    /// Accumulate a single API call's usage.
    ///
    /// Also syncs the cost to the global ProcessState for cross-module access.
    pub fn add_usage(&mut self, usage: &Usage, cost_usd: f64) {
        self.total_input_tokens += usage.input_tokens;
        self.total_output_tokens += usage.output_tokens;
        self.total_cache_read_tokens += usage.cache_read_input_tokens;
        self.total_cache_creation_tokens += usage.cache_creation_input_tokens;
        self.total_cost_usd += cost_usd;
        self.api_call_count += 1;

        // Sync to global ProcessState
        if let Ok(mut state) = crate::bootstrap::PROCESS_STATE.write() {
            state.total_cost_usd += cost_usd;
        }
    }
}

/// A record of a permission denial.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionDenial {
    pub tool_name: String,
    pub tool_use_id: String,
    pub reason: String,
    pub timestamp: i64,
}

/// Reason for aborting a query.
#[derive(Debug, Clone)]
pub enum AbortReason {
    /// User pressed Ctrl-C or called abort().
    UserAbort,
    /// Max budget exceeded.
    MaxBudget { spent_usd: f64, limit_usd: f64 },
    /// Max turns exceeded.
    MaxTurns { turns: usize, limit: usize },
    /// Unrecoverable API error.
    ApiError { message: String },
}

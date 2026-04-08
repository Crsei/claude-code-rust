#![allow(unused)]
//! Retry logic with exponential backoff and model fallback
use anyhow::Result;
use std::time::Duration;

/// Retry configuration
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: usize,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub retryable_status_codes: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            retryable_status_codes: vec![429, 500, 502, 503, 529],
        }
    }
}

/// Categorized API error for retry decisions
#[derive(Debug, Clone)]
pub enum ApiErrorCategory {
    /// Rate limited — retry with backoff
    RateLimit { retry_after_ms: Option<u64> },
    /// Server overloaded — retry with backoff, maybe fallback
    Overloaded,
    /// Server error — retry
    ServerError,
    /// Invalid request — don't retry
    InvalidRequest { message: String },
    /// Auth error — don't retry
    AuthError,
    /// Prompt too long — don't retry (handle differently)
    PromptTooLong,
    /// Max output tokens — don't retry (handle differently)
    MaxOutputTokens,
    /// Unknown — don't retry
    Unknown {
        status: Option<u16>,
        message: String,
    },
}

impl ApiErrorCategory {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. } | Self::Overloaded | Self::ServerError
        )
    }
}

/// Categorize an API error response
pub fn categorize_api_error(status: u16, body: &str) -> ApiErrorCategory {
    match status {
        429 => {
            // Check for overloaded vs rate limit
            if body.contains("overloaded") {
                ApiErrorCategory::Overloaded
            } else {
                ApiErrorCategory::RateLimit {
                    retry_after_ms: None,
                }
            }
        }
        500 | 502 | 503 => ApiErrorCategory::ServerError,
        529 => ApiErrorCategory::Overloaded,
        400 => {
            if body.contains("prompt is too long") || body.contains("too many tokens") {
                ApiErrorCategory::PromptTooLong
            } else if body.contains("max_tokens") {
                ApiErrorCategory::MaxOutputTokens
            } else {
                ApiErrorCategory::InvalidRequest {
                    message: body.to_string(),
                }
            }
        }
        401 | 403 => ApiErrorCategory::AuthError,
        _ => ApiErrorCategory::Unknown {
            status: Some(status),
            message: body.to_string(),
        },
    }
}

/// Calculate delay for a retry attempt
pub fn retry_delay(config: &RetryConfig, attempt: usize) -> Duration {
    let delay = config.initial_delay_ms as f64 * config.backoff_multiplier.powi(attempt as i32);
    let delay = delay.min(config.max_delay_ms as f64) as u64;
    // Add jitter (±20%)
    let jitter = (delay as f64 * 0.2 * (rand_fraction() * 2.0 - 1.0)) as i64;
    Duration::from_millis((delay as i64 + jitter).max(0) as u64)
}

fn rand_fraction() -> f64 {
    // Simple pseudo-random for jitter — not crypto-secure
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos % 1000) as f64 / 1000.0
}

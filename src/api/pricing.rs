//! Model token pricing — built-in table + environment variable overrides.
//!
//! Pricing sources (April 2026):
//!   - Anthropic: https://docs.anthropic.com/en/docs/about-claude/models
//!   - OpenAI:    https://openai.com/api/pricing/
//!   - Google:    https://ai.google.dev/pricing
//!   - DeepSeek:  https://api-docs.deepseek.com/
//!
//! Resolution order for a given model:
//!   1. Environment variables `MODEL_INPUT_PRICE` / `MODEL_OUTPUT_PRICE` (USD per 1M tokens)
//!   2. Built-in pricing table (exact match, then prefix match)
//!   3. Falls back to 0.0 (unknown model)

use crate::types::message::Usage;

/// Price per 1 million tokens (USD).
#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
}

impl ModelPricing {
    pub const ZERO: ModelPricing = ModelPricing {
        input_per_1m: 0.0,
        output_per_1m: 0.0,
    };

    /// Calculate cost in USD from a Usage struct.
    pub fn cost(&self, usage: &Usage) -> f64 {
        let input_cost = usage.input_tokens as f64 * self.input_per_1m / 1_000_000.0;
        let output_cost = usage.output_tokens as f64 * self.output_per_1m / 1_000_000.0;
        // Cache reads are typically cheaper; use input price as approximation
        let cache_read_cost =
            usage.cache_read_input_tokens as f64 * self.input_per_1m * 0.1 / 1_000_000.0;
        let cache_write_cost =
            usage.cache_creation_input_tokens as f64 * self.input_per_1m * 1.25 / 1_000_000.0;
        input_cost + output_cost + cache_read_cost + cache_write_cost
    }
}

// ---------------------------------------------------------------------------
// Built-in pricing table (USD per 1M tokens, April 2026)
// ---------------------------------------------------------------------------

/// (model_id_prefix, input_per_1m, output_per_1m)
const BUILTIN_PRICING: &[(&str, f64, f64)] = &[
    // ── Anthropic ──────────────────────────────────────────────
    ("claude-opus-4-6", 5.0, 25.0),
    ("claude-opus-4-0", 15.0, 75.0),
    ("claude-opus-4", 5.0, 25.0),
    ("claude-sonnet-4-6", 3.0, 15.0),
    ("claude-sonnet-4-0", 3.0, 15.0),
    ("claude-sonnet-4", 3.0, 15.0),
    ("claude-haiku-4-5", 1.0, 5.0),
    ("claude-haiku-3-5", 1.0, 5.0),
    // ── OpenAI ─────────────────────────────────────────────────
    ("gpt-4o-mini", 0.15, 0.60),
    ("gpt-4o", 2.50, 10.0),
    ("gpt-4.1-nano", 0.10, 0.40),
    ("gpt-4.1-mini", 0.20, 0.80),
    ("gpt-4.1", 2.0, 8.0),
    ("gpt-5-nano", 0.10, 0.40), // Azure custom deployment (estimated ≈ 4.1-nano)
    ("o4-mini", 0.55, 2.20),
    ("o3-mini", 1.10, 4.40),
    ("o3", 2.0, 8.0),
    // ── Google ─────────────────────────────────────────────────
    ("gemini-2.5-pro", 1.25, 10.0),
    ("gemini-2.5-flash", 0.30, 2.50),
    ("gemini-2.0-flash", 0.10, 0.40),
    // ── DeepSeek ───────────────────────────────────────────────
    ("deepseek-chat", 0.28, 0.42),
    ("deepseek-reasoner", 0.55, 2.19),
];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Look up pricing for a model.
///
/// Resolution:
///   1. `MODEL_INPUT_PRICE` / `MODEL_OUTPUT_PRICE` env vars (global override)
///   2. Built-in table: exact match on model id (case-insensitive)
///   3. Built-in table: prefix match (e.g. "gpt-4o-2024-..." matches "gpt-4o")
///   4. `ModelPricing::ZERO`
pub fn get_pricing(model: &str) -> ModelPricing {
    // 1. Environment variable override
    if let Some(pricing) = pricing_from_env() {
        return pricing;
    }

    // 2. Built-in table
    if let Some(pricing) = pricing_from_table(model) {
        return pricing;
    }

    // 3. Unknown model
    ModelPricing::ZERO
}

/// Calculate total cost in USD for a model + usage pair.
pub fn calculate_cost(model: &str, usage: &Usage) -> f64 {
    get_pricing(model).cost(usage)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn pricing_from_env() -> Option<ModelPricing> {
    let input = std::env::var("MODEL_INPUT_PRICE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())?;
    let output = std::env::var("MODEL_OUTPUT_PRICE")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())?;
    Some(ModelPricing {
        input_per_1m: input,
        output_per_1m: output,
    })
}

fn pricing_from_table(model: &str) -> Option<ModelPricing> {
    let lower = model.to_lowercase();

    // Exact match
    for &(prefix, input, output) in BUILTIN_PRICING {
        if lower == prefix {
            return Some(ModelPricing {
                input_per_1m: input,
                output_per_1m: output,
            });
        }
    }

    // Prefix match (longer prefixes checked first via table order)
    for &(prefix, input, output) in BUILTIN_PRICING {
        if lower.starts_with(prefix) {
            return Some(ModelPricing {
                input_per_1m: input,
                output_per_1m: output,
            });
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::Usage;

    #[test]
    fn exact_match_claude_sonnet() {
        let p = pricing_from_table("claude-sonnet-4-20250514").unwrap();
        assert_eq!(p.input_per_1m, 3.0);
        assert_eq!(p.output_per_1m, 15.0);
    }

    #[test]
    fn exact_match_gpt4o() {
        let p = pricing_from_table("gpt-4o").unwrap();
        assert_eq!(p.input_per_1m, 2.5);
        assert_eq!(p.output_per_1m, 10.0);
    }

    #[test]
    fn prefix_match_gpt4o_dated() {
        let p = pricing_from_table("gpt-4o-2024-11-20").unwrap();
        assert_eq!(p.input_per_1m, 2.5);
    }

    #[test]
    fn gpt5_nano_known() {
        let p = pricing_from_table("gpt-5-nano").unwrap();
        assert_eq!(p.input_per_1m, 0.10);
        assert_eq!(p.output_per_1m, 0.40);
    }

    #[test]
    fn unknown_model_returns_none() {
        assert!(pricing_from_table("totally-unknown-model").is_none());
    }

    #[test]
    fn case_insensitive() {
        let p = pricing_from_table("Claude-Sonnet-4-20250514").unwrap();
        assert_eq!(p.input_per_1m, 3.0);
    }

    #[test]
    fn calculate_cost_basic() {
        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        // gpt-4o: $2.50/1M input, $10/1M output
        let cost = calculate_cost("gpt-4o", &usage);
        let expected = 1000.0 * 2.5 / 1_000_000.0 + 500.0 * 10.0 / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-10);
    }

    #[test]
    fn calculate_cost_unknown_model_is_zero() {
        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        assert_eq!(calculate_cost("unknown-model", &usage), 0.0);
    }

    #[test]
    fn env_override_takes_precedence() {
        // Save originals
        let orig_input = std::env::var("MODEL_INPUT_PRICE").ok();
        let orig_output = std::env::var("MODEL_OUTPUT_PRICE").ok();

        std::env::set_var("MODEL_INPUT_PRICE", "99.0");
        std::env::set_var("MODEL_OUTPUT_PRICE", "199.0");

        let p = get_pricing("gpt-4o"); // would normally be 2.5/10.0
        assert_eq!(p.input_per_1m, 99.0);
        assert_eq!(p.output_per_1m, 199.0);

        // Restore originals
        match orig_input {
            Some(v) => std::env::set_var("MODEL_INPUT_PRICE", v),
            None => std::env::remove_var("MODEL_INPUT_PRICE"),
        }
        match orig_output {
            Some(v) => std::env::set_var("MODEL_OUTPUT_PRICE", v),
            None => std::env::remove_var("MODEL_OUTPUT_PRICE"),
        }
    }
}

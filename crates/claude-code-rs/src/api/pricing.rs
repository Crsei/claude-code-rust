//! Runtime cost helpers backed by model pricing metadata from `cc-models`.

use crate::types::message::Usage;

/// Calculate total cost in USD for a model + usage pair.
pub fn calculate_cost(model: &str, usage: &Usage) -> f64 {
    cc_models::get_pricing(model).cost_from_counts(
        usage.input_tokens,
        usage.output_tokens,
        usage.cache_read_input_tokens,
        usage.cache_creation_input_tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_cost_basic() {
        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        let cost = calculate_cost("gpt-4o", &usage);
        let expected = 1000.0 * 2.5 / 1_000_000.0 + 500.0 * 10.0 / 1_000_000.0;
        assert!((cost - expected).abs() < 1e-10);
    }

    #[test]
    fn calculate_cost_unknown_model_is_zero() {
        let orig_input = std::env::var("MODEL_INPUT_PRICE").ok();
        let orig_output = std::env::var("MODEL_OUTPUT_PRICE").ok();
        std::env::remove_var("MODEL_INPUT_PRICE");
        std::env::remove_var("MODEL_OUTPUT_PRICE");

        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        };
        let cost = calculate_cost("unknown-model", &usage);

        match orig_input {
            Some(v) => std::env::set_var("MODEL_INPUT_PRICE", v),
            None => std::env::remove_var("MODEL_INPUT_PRICE"),
        }
        match orig_output {
            Some(v) => std::env::set_var("MODEL_OUTPUT_PRICE", v),
            None => std::env::remove_var("MODEL_OUTPUT_PRICE"),
        }
        assert_eq!(cost, 0.0);
    }
}

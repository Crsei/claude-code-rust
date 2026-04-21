//! Claude model ID mapping across providers (first-party, Bedrock, Vertex).
//!
//! cc-rust uses first-party Claude model IDs (e.g. `claude-sonnet-4-5-20250929`)
//! internally. When talking to AWS Bedrock or GCP Vertex AI, these IDs need to
//! be translated to provider-specific strings:
//!
//! - Bedrock: `us.anthropic.claude-sonnet-4-5-20250929-v1:0`
//! - Vertex:  `claude-sonnet-4-5@20250929`
//!
//! Reference: claude-code-bun `src/utils/model/configs.ts`.

/// A single Claude model's IDs across providers.
#[derive(Debug, Clone, Copy)]
pub struct ModelConfig {
    pub first_party: &'static str,
    pub bedrock: &'static str,
    pub vertex: &'static str,
}

/// All known Claude model configs.
///
/// Order matters: mapping lookups use the first match for substring matching
/// when the input is already in a provider-specific format.
pub const CLAUDE_MODELS: &[ModelConfig] = &[
    ModelConfig {
        first_party: "claude-opus-4-6",
        bedrock: "us.anthropic.claude-opus-4-6-v1",
        vertex: "claude-opus-4-6",
    },
    ModelConfig {
        first_party: "claude-sonnet-4-6",
        bedrock: "us.anthropic.claude-sonnet-4-6",
        vertex: "claude-sonnet-4-6",
    },
    ModelConfig {
        first_party: "claude-opus-4-5-20251101",
        bedrock: "us.anthropic.claude-opus-4-5-20251101-v1:0",
        vertex: "claude-opus-4-5@20251101",
    },
    ModelConfig {
        first_party: "claude-opus-4-1-20250805",
        bedrock: "us.anthropic.claude-opus-4-1-20250805-v1:0",
        vertex: "claude-opus-4-1@20250805",
    },
    ModelConfig {
        first_party: "claude-opus-4-20250514",
        bedrock: "us.anthropic.claude-opus-4-20250514-v1:0",
        vertex: "claude-opus-4@20250514",
    },
    ModelConfig {
        first_party: "claude-sonnet-4-5-20250929",
        bedrock: "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
        vertex: "claude-sonnet-4-5@20250929",
    },
    ModelConfig {
        first_party: "claude-sonnet-4-20250514",
        bedrock: "us.anthropic.claude-sonnet-4-20250514-v1:0",
        vertex: "claude-sonnet-4@20250514",
    },
    ModelConfig {
        first_party: "claude-haiku-4-5-20251001",
        bedrock: "us.anthropic.claude-haiku-4-5-20251001-v1:0",
        vertex: "claude-haiku-4-5@20251001",
    },
    ModelConfig {
        first_party: "claude-3-7-sonnet-20250219",
        bedrock: "us.anthropic.claude-3-7-sonnet-20250219-v1:0",
        vertex: "claude-3-7-sonnet@20250219",
    },
    ModelConfig {
        first_party: "claude-3-5-sonnet-20241022",
        bedrock: "anthropic.claude-3-5-sonnet-20241022-v2:0",
        vertex: "claude-3-5-sonnet-v2@20241022",
    },
    ModelConfig {
        first_party: "claude-3-5-haiku-20241022",
        bedrock: "us.anthropic.claude-3-5-haiku-20241022-v1:0",
        vertex: "claude-3-5-haiku@20241022",
    },
];

/// Translate a first-party Claude model ID to its Bedrock equivalent.
///
/// If the input is already in Bedrock format (contains `anthropic.` or a known
/// region prefix), it is returned unchanged. Unknown models are returned as-is
/// so users can pass custom inference profile ARNs or IDs.
pub fn to_bedrock_model_id(model: &str) -> String {
    if looks_like_bedrock_id(model) {
        return model.to_string();
    }
    for cfg in CLAUDE_MODELS {
        if cfg.first_party == model {
            return cfg.bedrock.to_string();
        }
    }
    model.to_string()
}

/// Translate a first-party Claude model ID to its Vertex equivalent.
///
/// If the input already contains `@` (Vertex version separator), it is returned
/// unchanged. Unknown models are returned as-is.
pub fn to_vertex_model_id(model: &str) -> String {
    if model.contains('@') || model.starts_with("projects/") {
        return model.to_string();
    }
    for cfg in CLAUDE_MODELS {
        if cfg.first_party == model {
            return cfg.vertex.to_string();
        }
    }
    model.to_string()
}

/// Return true if `model` looks like it's already in Bedrock format.
fn looks_like_bedrock_id(model: &str) -> bool {
    if model.starts_with("arn:") {
        return true;
    }
    if model.starts_with("anthropic.") {
        return true;
    }
    // Region-prefixed cross-region inference profiles (us/eu/apac/global).
    for prefix in ["us.", "eu.", "apac.", "global."] {
        if model.starts_with(prefix) && model[prefix.len()..].starts_with("anthropic.") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_party_to_bedrock_sonnet45() {
        assert_eq!(
            to_bedrock_model_id("claude-sonnet-4-5-20250929"),
            "us.anthropic.claude-sonnet-4-5-20250929-v1:0"
        );
    }

    #[test]
    fn first_party_to_bedrock_haiku45() {
        assert_eq!(
            to_bedrock_model_id("claude-haiku-4-5-20251001"),
            "us.anthropic.claude-haiku-4-5-20251001-v1:0"
        );
    }

    #[test]
    fn bedrock_passthrough_region_prefix() {
        assert_eq!(
            to_bedrock_model_id("eu.anthropic.claude-sonnet-4-5-20250929-v1:0"),
            "eu.anthropic.claude-sonnet-4-5-20250929-v1:0"
        );
    }

    #[test]
    fn bedrock_passthrough_foundation_model() {
        assert_eq!(
            to_bedrock_model_id("anthropic.claude-3-5-sonnet-20241022-v2:0"),
            "anthropic.claude-3-5-sonnet-20241022-v2:0"
        );
    }

    #[test]
    fn bedrock_passthrough_arn() {
        let arn = "arn:aws:bedrock:us-east-1:123:inference-profile/claude-opus-4";
        assert_eq!(to_bedrock_model_id(arn), arn);
    }

    #[test]
    fn first_party_to_vertex_sonnet45() {
        assert_eq!(
            to_vertex_model_id("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5@20250929"
        );
    }

    #[test]
    fn first_party_to_vertex_opus46() {
        assert_eq!(to_vertex_model_id("claude-opus-4-6"), "claude-opus-4-6");
    }

    #[test]
    fn vertex_passthrough_at_separator() {
        assert_eq!(
            to_vertex_model_id("claude-sonnet-4-5@20250929"),
            "claude-sonnet-4-5@20250929"
        );
    }

    #[test]
    fn unknown_model_passthrough() {
        assert_eq!(to_bedrock_model_id("custom-model-id"), "custom-model-id");
        assert_eq!(to_vertex_model_id("custom-model-id"), "custom-model-id");
    }
}

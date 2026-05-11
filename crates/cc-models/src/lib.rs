//! Shared model metadata for cc-rust.

pub mod aliases;
pub mod mapping;
pub mod pricing;
pub mod setting;

pub use aliases::{
    alias_for_model, is_removed_legacy_model_alias, removed_legacy_model_alias_error,
    replacement_for_removed_legacy_alias, resolve_model_alias, ModelAlias, FOTA_MODEL_ID,
    MODEL_ALIASES, MOTA_MODEL_ID, REMOVED_LEGACY_MODEL_ALIASES, SOTA_MODEL_ID,
};
pub use mapping::{to_bedrock_model_id, to_vertex_model_id, ModelConfig, CLAUDE_MODELS};
pub use pricing::{cost_from_counts, get_pricing, ModelPricing};
pub use setting::{ModelSetting, ModelStrings, ModelTier};

//! Neutral public model aliases for cc-rust.
//!
//! These aliases are intentionally project-local. They give UI and command
//! surfaces stable capability labels while the API layer still receives the
//! concrete provider model IDs.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelAlias {
    pub alias: &'static str,
    pub target: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

pub const SOTA_MODEL_ID: &str = "claude-opus-4-20250514";
pub const MOTA_MODEL_ID: &str = "claude-sonnet-4-20250514";
pub const FOTA_MODEL_ID: &str = "claude-haiku-3-5-20241022";

pub const MODEL_ALIASES: &[ModelAlias] = &[
    ModelAlias {
        alias: "SOTA",
        target: SOTA_MODEL_ID,
        label: "SOTA",
        description: "Highest capability work",
    },
    ModelAlias {
        alias: "MOTA",
        target: MOTA_MODEL_ID,
        label: "MOTA",
        description: "Balanced capability and speed",
    },
    ModelAlias {
        alias: "FOTA",
        target: FOTA_MODEL_ID,
        label: "FOTA",
        description: "Fast lightweight work",
    },
];

pub const REMOVED_LEGACY_MODEL_ALIASES: &[(&str, &str)] =
    &[("opus", "SOTA"), ("sonnet", "MOTA"), ("haiku", "FOTA")];

pub fn resolve_model_alias(name: &str) -> String {
    let trimmed = name.trim();
    MODEL_ALIASES
        .iter()
        .find(|entry| trimmed.eq_ignore_ascii_case(entry.alias))
        .map(|entry| entry.target.to_string())
        .unwrap_or_else(|| trimmed.to_string())
}

pub fn alias_for_model(model: &str) -> Option<&'static str> {
    MODEL_ALIASES
        .iter()
        .find_map(|entry| (entry.target == model).then_some(entry.alias))
}

pub fn replacement_for_removed_legacy_alias(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    REMOVED_LEGACY_MODEL_ALIASES
        .iter()
        .find_map(|(legacy, replacement)| {
            trimmed.eq_ignore_ascii_case(legacy).then_some(*replacement)
        })
}

pub fn is_removed_legacy_model_alias(name: &str) -> bool {
    replacement_for_removed_legacy_alias(name).is_some()
}

pub fn removed_legacy_model_alias_error(name: &str) -> String {
    let trimmed = name.trim();
    let replacement = replacement_for_removed_legacy_alias(trimmed).unwrap_or("MOTA");
    format!(
        "Legacy model alias '{}' is no longer supported in cc-rust. Use '{}' or a full model ID instead.",
        trimmed, replacement
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_aliases_resolve_to_existing_model_ids() {
        assert_eq!(resolve_model_alias("SOTA"), SOTA_MODEL_ID);
        assert_eq!(resolve_model_alias("mota"), MOTA_MODEL_ID);
        assert_eq!(resolve_model_alias("FOTA"), FOTA_MODEL_ID);
    }

    #[test]
    fn legacy_family_names_are_not_public_aliases() {
        assert_eq!(resolve_model_alias("opus"), "opus");
        assert_eq!(resolve_model_alias("sonnet"), "sonnet");
        assert_eq!(resolve_model_alias("haiku"), "haiku");
        assert_eq!(replacement_for_removed_legacy_alias("opus"), Some("SOTA"));
        assert_eq!(replacement_for_removed_legacy_alias("sonnet"), Some("MOTA"));
        assert_eq!(replacement_for_removed_legacy_alias("haiku"), Some("FOTA"));
    }

    #[test]
    fn custom_model_ids_pass_through() {
        assert_eq!(resolve_model_alias("gpt-5.4"), "gpt-5.4");
        assert_eq!(resolve_model_alias(" deepseek-chat "), "deepseek-chat");
    }
}

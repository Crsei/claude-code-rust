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

pub fn resolve_model_alias(name: &str) -> String {
    let trimmed = name.trim();
    MODEL_ALIASES
        .iter()
        .find(|entry| trimmed.eq_ignore_ascii_case(entry.alias))
        .map(|entry| entry.target.to_string())
        .unwrap_or_else(|| trimmed.to_string())
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
    }

    #[test]
    fn custom_model_ids_pass_through() {
        assert_eq!(resolve_model_alias("gpt-5.4"), "gpt-5.4");
        assert_eq!(resolve_model_alias(" deepseek-chat "), "deepseek-chat");
    }
}

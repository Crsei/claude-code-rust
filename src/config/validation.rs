#![allow(dead_code)] // Public API — will be used by settings loader
//! Settings validation.
//!
//! Validates configuration values and reports warnings for
//! invalid or suspicious settings.

use anyhow::{bail, Result};

use crate::types::app_state::SettingsJson;

const VALID_BACKENDS: &[&str] = &["native", "codex"];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Severity level for validation warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningSeverity {
    /// Informational note, not a problem.
    Info,
    /// Something looks off but may still work.
    Warning,
    /// Invalid value that will cause failures.
    Error,
}

/// A single validation warning produced when checking settings.
#[derive(Debug, Clone)]
pub struct ValidationWarning {
    /// The setting field that triggered the warning.
    pub field: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// How serious the issue is.
    pub severity: WarningSeverity,
}

// ---------------------------------------------------------------------------
// Known model patterns
// ---------------------------------------------------------------------------

/// Prefixes that are recognized as valid Anthropic model identifiers.
const VALID_MODEL_PREFIXES: &[&str] = &["claude-", "anthropic."];

/// Specific model names that are always valid (aliases, etc.).
const VALID_MODEL_NAMES: &[&str] = &[
    "claude-opus-4-6-20250414",
    "claude-sonnet-4-20250514",
    "claude-haiku-4-5",
    "claude-3-opus-20240229",
    "claude-3-sonnet-20240229",
    "claude-3-haiku-20240307",
    "claude-3-5-sonnet-20241022",
    "claude-3-5-haiku-20241022",
];

/// Model prefixes from third-party providers (OpenAI, Google, etc.)
/// that we also accept.
const THIRD_PARTY_PREFIXES: &[&str] = &[
    "gpt-",
    "o1-",
    "o3-",
    "gemini-",
    "models/gemini-",
    "deepseek-",
    "mistral-",
    "codestral-",
    "command-",
    "accounts/", // Vertex AI paths
];

// ---------------------------------------------------------------------------
// Validation functions
// ---------------------------------------------------------------------------

/// Validate a model name.
///
/// Returns `Ok(())` if the model name matches a known pattern.
/// Returns an error if the name is clearly invalid.
///
/// Note: this is a best-effort check. Unknown but syntactically valid
/// model names are accepted to allow for new model releases.
pub fn validate_model_name(model: &str) -> Result<()> {
    let trimmed = model.trim();

    if trimmed.is_empty() {
        bail!("Model name cannot be empty");
    }

    if trimmed.len() > 256 {
        bail!("Model name is too long (max 256 characters)");
    }

    // Check for obviously invalid characters
    if trimmed.contains(char::is_whitespace) {
        bail!("Model name cannot contain whitespace: '{}'", trimmed);
    }

    // Accept exact known model names
    if VALID_MODEL_NAMES.contains(&trimmed) {
        return Ok(());
    }

    // Accept known prefixes
    let lower = trimmed.to_lowercase();
    for prefix in VALID_MODEL_PREFIXES
        .iter()
        .chain(THIRD_PARTY_PREFIXES.iter())
    {
        if lower.starts_with(prefix) {
            return Ok(());
        }
    }

    // Accept model names that look like provider paths (contain a `/`)
    if trimmed.contains('/') {
        return Ok(());
    }

    bail!(
        "Unrecognized model name: '{}'. Expected a model ID like \
         'claude-sonnet-4-20250514' or 'claude-opus-4-6-20250414'.",
        trimmed
    )
}

/// Validate all settings in a `SettingsJson` and return a list of warnings.
///
/// Does not fail hard -- returns warnings that the caller can display or log.
pub fn validate_settings(settings: &SettingsJson) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    // Validate model name
    if let Some(ref model) = settings.model {
        if model.trim().is_empty() {
            warnings.push(ValidationWarning {
                field: "model".to_string(),
                message: "Model name is set but empty. The default model will be used.".to_string(),
                severity: WarningSeverity::Warning,
            });
        } else if let Err(e) = validate_model_name(model) {
            warnings.push(ValidationWarning {
                field: "model".to_string(),
                message: format!("Invalid model: {}", e),
                severity: WarningSeverity::Error,
            });
        }
    }

    if let Some(ref backend) = settings.backend {
        let normalized = backend.trim().to_ascii_lowercase();
        if !normalized.is_empty() && !VALID_BACKENDS.contains(&normalized.as_str()) {
            warnings.push(ValidationWarning {
                field: "backend".to_string(),
                message: format!(
                    "Unknown backend '{}'. Known backends: {}.",
                    backend,
                    VALID_BACKENDS.join(", ")
                ),
                severity: WarningSeverity::Error,
            });
        }
    }

    // Validate theme
    if let Some(ref theme) = settings.theme {
        let known_themes = ["dark", "light", "auto", "solarized", "monokai", "nord"];
        if !theme.is_empty() && !known_themes.contains(&theme.to_lowercase().as_str()) {
            warnings.push(ValidationWarning {
                field: "theme".to_string(),
                message: format!(
                    "Unknown theme '{}'. Known themes: {}.",
                    theme,
                    known_themes.join(", ")
                ),
                severity: WarningSeverity::Info,
            });
        }
    }

    // Validate permission mode (legacy + nested)
    let permission_mode = settings
        .permissions
        .default_mode
        .as_ref()
        .or(settings.permission_mode.as_ref());
    if let Some(mode) = permission_mode {
        let valid = ["default", "ask", "auto", "bypass", "plan"];
        if !valid.contains(&mode.to_lowercase().as_str()) {
            warnings.push(ValidationWarning {
                field: "permissionMode".to_string(),
                message: format!(
                    "Unknown permission mode '{}'. Known modes: {}.",
                    mode,
                    valid.join(", ")
                ),
                severity: WarningSeverity::Error,
            });
        }
    }

    // Validate editor mode
    if let Some(mode) = &settings.editor_mode {
        let valid = ["normal", "vim"];
        if !valid.contains(&mode.to_lowercase().as_str()) {
            warnings.push(ValidationWarning {
                field: "editorMode".to_string(),
                message: format!(
                    "Unknown editor mode '{}'. Known modes: {}.",
                    mode,
                    valid.join(", ")
                ),
                severity: WarningSeverity::Warning,
            });
        }
    }

    // Validate language code looks like BCP-47 / a short identifier.
    if let Some(lang) = &settings.language {
        if lang.trim().is_empty() {
            warnings.push(ValidationWarning {
                field: "language".to_string(),
                message: "Language code is set but empty.".into(),
                severity: WarningSeverity::Warning,
            });
        }
    }

    warnings
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_anthropic_models() {
        assert!(validate_model_name("claude-opus-4-6-20250414").is_ok());
        assert!(validate_model_name("claude-sonnet-4-20250514").is_ok());
        assert!(validate_model_name("claude-haiku-4-5").is_ok());
        assert!(validate_model_name("claude-3-5-sonnet-20241022").is_ok());
        assert!(validate_model_name("claude-3-opus-20240229").is_ok());
    }

    #[test]
    fn test_valid_third_party_models() {
        assert!(validate_model_name("gpt-4o").is_ok());
        assert!(validate_model_name("o1-mini").is_ok());
        assert!(validate_model_name("gemini-1.5-pro").is_ok());
        assert!(validate_model_name("deepseek-chat").is_ok());
        assert!(validate_model_name("mistral-large-latest").is_ok());
    }

    #[test]
    fn test_valid_path_model() {
        assert!(validate_model_name("accounts/my-project/models/claude-v1").is_ok());
        assert!(validate_model_name("models/gemini-1.5-flash").is_ok());
    }

    #[test]
    fn test_invalid_models() {
        assert!(validate_model_name("").is_err());
        assert!(validate_model_name("   ").is_err());
        assert!(validate_model_name("my model name").is_err());
        assert!(validate_model_name("totally-random-string").is_err());
    }

    #[test]
    fn test_model_name_too_long() {
        let long_name = "claude-".to_string() + &"x".repeat(300);
        assert!(validate_model_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_settings_empty() {
        let settings = SettingsJson::default();
        let warnings = validate_settings(&settings);
        assert!(
            warnings.is_empty(),
            "Default settings should have no warnings"
        );
    }

    #[test]
    fn test_validate_settings_bad_model() {
        let settings = SettingsJson {
            model: Some("totally invalid model".to_string()),
            ..Default::default()
        };
        let warnings = validate_settings(&settings);
        assert!(!warnings.is_empty());
        assert_eq!(warnings[0].field, "model");
        assert_eq!(warnings[0].severity, WarningSeverity::Error);
    }

    #[test]
    fn test_validate_settings_empty_model() {
        let settings = SettingsJson {
            model: Some("".to_string()),
            ..Default::default()
        };
        let warnings = validate_settings(&settings);
        assert!(!warnings.is_empty());
        assert_eq!(warnings[0].severity, WarningSeverity::Warning);
    }

    #[test]
    fn test_validate_settings_unknown_theme() {
        let settings = SettingsJson {
            theme: Some("cyberpunk".to_string()),
            ..Default::default()
        };
        let warnings = validate_settings(&settings);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].field, "theme");
        assert_eq!(warnings[0].severity, WarningSeverity::Info);
    }

    #[test]
    fn test_validate_settings_good() {
        let settings = SettingsJson {
            model: Some("claude-sonnet-4-20250514".to_string()),
            backend: Some("codex".to_string()),
            theme: Some("dark".to_string()),
            verbose: Some(true),
            ..Default::default()
        };
        let warnings = validate_settings(&settings);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_validate_settings_bad_backend() {
        let settings = SettingsJson {
            backend: Some("mystery".to_string()),
            ..Default::default()
        };
        let warnings = validate_settings(&settings);
        assert!(!warnings.is_empty());
        assert_eq!(warnings[0].field, "backend");
        assert_eq!(warnings[0].severity, WarningSeverity::Error);
    }

    #[test]
    fn test_validate_settings_bad_permission_mode() {
        let settings = SettingsJson {
            permission_mode: Some("nope".into()),
            ..Default::default()
        };
        let warnings = validate_settings(&settings);
        assert!(warnings.iter().any(|w| w.field == "permissionMode"
            && w.severity == WarningSeverity::Error));
    }

    #[test]
    fn test_validate_settings_bad_editor_mode() {
        let settings = SettingsJson {
            editor_mode: Some("emacs".into()),
            ..Default::default()
        };
        let warnings = validate_settings(&settings);
        assert!(warnings.iter().any(|w| w.field == "editorMode"));
    }
}

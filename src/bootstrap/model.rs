//! 结构化模型配置类型
//!
//! 对应 TypeScript:
//! - bootstrap/src/utils/model/model.ts (ModelSetting)
//! - bootstrap/src/utils/model/modelStrings.ts (ModelStrings)

/// 模型层级
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelTier {
    Opus,
    Sonnet,
    Haiku,
    Unknown,
}

impl ModelTier {
    pub fn as_str(&self) -> &str {
        match self {
            ModelTier::Opus => "opus",
            ModelTier::Sonnet => "sonnet",
            ModelTier::Haiku => "haiku",
            ModelTier::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 结构化模型配置 — 从原始 model ID 推断层级和显示名
#[derive(Debug, Clone)]
pub struct ModelSetting {
    /// 原始模型 ID (e.g. "claude-sonnet-4-20250514")
    pub model_id: String,
    /// 显示名称 (e.g. "Sonnet 4")
    pub display_name: String,
    /// 层级
    pub tier: ModelTier,
}

impl ModelSetting {
    /// 从原始模型 ID 推断 tier 和显示名。
    pub fn from_model_id(model_id: &str) -> Self {
        let lower = model_id.to_lowercase();

        let (tier, display_name) = if lower.contains("opus") {
            (ModelTier::Opus, "Opus")
        } else if lower.contains("sonnet") {
            (ModelTier::Sonnet, "Sonnet")
        } else if lower.contains("haiku") {
            (ModelTier::Haiku, "Haiku")
        } else {
            (ModelTier::Unknown, model_id)
        };

        Self {
            model_id: model_id.to_string(),
            display_name: display_name.to_string(),
            tier,
        }
    }
}

/// 模型字符串集合 — 用于 UI 显示和日志
#[derive(Debug, Clone)]
pub struct ModelStrings {
    /// 主模型完整 ID
    pub main_model_id: String,
    /// 主模型显示名
    pub main_display: String,
    /// 快速模式模型显示名 (如果配置了)
    pub fast_display: Option<String>,
}

impl ModelStrings {
    /// 从 ModelSetting 构造。
    pub fn from_setting(setting: &ModelSetting) -> Self {
        Self {
            main_model_id: setting.model_id.clone(),
            main_display: setting.display_name.clone(),
            fast_display: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_sonnet() {
        let s = ModelSetting::from_model_id("claude-sonnet-4-20250514");
        assert_eq!(s.tier, ModelTier::Sonnet);
        assert_eq!(s.display_name, "Sonnet");
    }

    #[test]
    fn detect_opus() {
        let s = ModelSetting::from_model_id("claude-opus-4-20250514");
        assert_eq!(s.tier, ModelTier::Opus);
    }

    #[test]
    fn detect_haiku() {
        let s = ModelSetting::from_model_id("claude-haiku-4-5-20251001");
        assert_eq!(s.tier, ModelTier::Haiku);
    }

    #[test]
    fn detect_unknown() {
        let s = ModelSetting::from_model_id("gpt-4o");
        assert_eq!(s.tier, ModelTier::Unknown);
        assert_eq!(s.display_name, "gpt-4o");
    }

    #[test]
    fn model_strings_from_setting() {
        let setting = ModelSetting::from_model_id("claude-opus-4-20250514");
        let strings = ModelStrings::from_setting(&setting);
        assert_eq!(strings.main_display, "Opus");
        assert!(strings.fast_display.is_none());
    }
}

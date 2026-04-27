//! Feature gate system for KAIROS and related features.
//!
//! Each feature is controlled by an environment variable (`FEATURE_*`).
//! Dependency rules enforce that child features require their parent:
//! - `kairos_brief`, `kairos_channels`, `kairos_push_notification`,
//!   `kairos_github_webhooks` all require `kairos`.
//! - `proactive` can be standalone OR is implied when `kairos` is enabled.
//!
//! A global singleton [`FLAGS`] is lazily initialised from real env vars.
//! Use [`enabled`] for quick queries from anywhere in the crate.

use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// Feature enum
// ---------------------------------------------------------------------------

/// Individual feature variants used for runtime queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    Kairos,
    KairosBrief,
    KairosChannels,
    KairosPushNotification,
    #[allow(dead_code)]
    KairosGithubWebhooks,
    Proactive,
    TeamMemory,
    SubagentDashboard,
}

// ---------------------------------------------------------------------------
// FeatureFlags
// ---------------------------------------------------------------------------

/// Resolved set of feature flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlags {
    pub kairos: bool,
    pub kairos_brief: bool,
    pub kairos_channels: bool,
    pub kairos_push_notification: bool,
    pub kairos_github_webhooks: bool,
    pub proactive: bool,
    pub team_memory: bool,
    pub subagent_dashboard: bool,
}

impl FeatureFlags {
    /// Build flags from real environment variables.
    pub fn from_env() -> Self {
        Self::from_env_iter(std::env::vars())
    }

    /// Build flags from an iterator of `(key, value)` pairs.
    ///
    /// A variable is considered *enabled* when its value, after trimming and
    /// lowercasing, is `"1"` or `"true"`.  Anything else (including absence)
    /// is treated as disabled.
    pub fn from_env_iter(iter: impl IntoIterator<Item = (String, String)>) -> Self {
        use std::collections::HashMap;
        let env: HashMap<String, String> = iter.into_iter().collect();

        let read = |key: &str| -> bool {
            env.get(key)
                .map(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true"))
                .unwrap_or(false)
        };

        let kairos = read("FEATURE_KAIROS");
        let team_memory = read("FEATURE_TEAMMEM");
        let subagent_dashboard = read("FEATURE_SUBAGENT_DASHBOARD");
        let mut kairos_brief = read("FEATURE_KAIROS_BRIEF");
        let mut kairos_channels = read("FEATURE_KAIROS_CHANNELS");
        let mut kairos_push_notification = read("FEATURE_KAIROS_PUSH_NOTIFICATION");
        let mut kairos_github_webhooks = read("FEATURE_KAIROS_GITHUB_WEBHOOKS");
        let mut proactive = read("FEATURE_PROACTIVE");

        // --- dependency enforcement ---
        // Children require `kairos`.  Disable + warn when parent is missing.
        if !kairos {
            if kairos_brief {
                tracing::warn!(
                    "FEATURE_KAIROS_BRIEF is set but FEATURE_KAIROS is not enabled; \
                     disabling kairos_brief"
                );
                kairos_brief = false;
            }
            if kairos_channels {
                tracing::warn!(
                    "FEATURE_KAIROS_CHANNELS is set but FEATURE_KAIROS is not enabled; \
                     disabling kairos_channels"
                );
                kairos_channels = false;
            }
            if kairos_push_notification {
                tracing::warn!(
                    "FEATURE_KAIROS_PUSH_NOTIFICATION is set but FEATURE_KAIROS is not enabled; \
                     disabling kairos_push_notification"
                );
                kairos_push_notification = false;
            }
            if kairos_github_webhooks {
                tracing::warn!(
                    "FEATURE_KAIROS_GITHUB_WEBHOOKS is set but FEATURE_KAIROS is not enabled; \
                     disabling kairos_github_webhooks"
                );
                kairos_github_webhooks = false;
            }
        }

        // `proactive` is implied when `kairos` is on.
        if kairos {
            proactive = true;
        }

        Self {
            kairos,
            kairos_brief,
            kairos_channels,
            kairos_push_notification,
            kairos_github_webhooks,
            proactive,
            team_memory,
            subagent_dashboard,
        }
    }

    /// Query whether a specific [`Feature`] is enabled.
    pub fn is_enabled(&self, feature: Feature) -> bool {
        match feature {
            Feature::Kairos => self.kairos,
            Feature::KairosBrief => self.kairos_brief,
            Feature::KairosChannels => self.kairos_channels,
            Feature::KairosPushNotification => self.kairos_push_notification,
            Feature::KairosGithubWebhooks => self.kairos_github_webhooks,
            Feature::Proactive => self.proactive,
            Feature::TeamMemory => self.team_memory,
            Feature::SubagentDashboard => self.subagent_dashboard,
        }
    }
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

/// Global feature flags initialised once from environment variables.
pub static FLAGS: LazyLock<FeatureFlags> = LazyLock::new(FeatureFlags::from_env);

/// Convenience: query the global singleton for a specific feature.
pub fn enabled(feature: Feature) -> bool {
    FLAGS.is_enabled(feature)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build flags from a slice of `(&str, &str)` pairs.
    fn flags(pairs: &[(&str, &str)]) -> FeatureFlags {
        FeatureFlags::from_env_iter(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())))
    }

    #[test]
    fn defaults_all_false() {
        let f = flags(&[]);
        assert!(!f.kairos);
        assert!(!f.kairos_brief);
        assert!(!f.kairos_channels);
        assert!(!f.kairos_push_notification);
        assert!(!f.kairos_github_webhooks);
        assert!(!f.proactive);
        assert!(!f.subagent_dashboard);
    }

    #[test]
    fn kairos_enables_proactive() {
        let f = flags(&[("FEATURE_KAIROS", "1")]);
        assert!(f.kairos);
        assert!(f.proactive, "proactive should be implied by kairos");
    }

    #[test]
    fn proactive_standalone() {
        let f = flags(&[("FEATURE_PROACTIVE", "true")]);
        assert!(!f.kairos);
        assert!(f.proactive, "proactive should work standalone");
    }

    #[test]
    fn brief_without_kairos_is_disabled() {
        let f = flags(&[("FEATURE_KAIROS_BRIEF", "1")]);
        assert!(
            !f.kairos_brief,
            "kairos_brief must be disabled when kairos is off"
        );
    }

    #[test]
    fn channels_without_kairos_is_disabled() {
        let f = flags(&[("FEATURE_KAIROS_CHANNELS", "true")]);
        assert!(
            !f.kairos_channels,
            "kairos_channels must be disabled when kairos is off"
        );
    }

    #[test]
    fn push_notification_without_kairos_is_disabled() {
        let f = flags(&[("FEATURE_KAIROS_PUSH_NOTIFICATION", "1")]);
        assert!(
            !f.kairos_push_notification,
            "kairos_push_notification must be disabled when kairos is off"
        );
    }

    #[test]
    fn github_webhooks_without_kairos_is_disabled() {
        let f = flags(&[("FEATURE_KAIROS_GITHUB_WEBHOOKS", "1")]);
        assert!(
            !f.kairos_github_webhooks,
            "kairos_github_webhooks must be disabled when kairos is off"
        );
    }

    #[test]
    fn children_enabled_when_kairos_on() {
        let f = flags(&[
            ("FEATURE_KAIROS", "1"),
            ("FEATURE_KAIROS_BRIEF", "true"),
            ("FEATURE_KAIROS_CHANNELS", "1"),
            ("FEATURE_KAIROS_PUSH_NOTIFICATION", "TRUE"),
            ("FEATURE_KAIROS_GITHUB_WEBHOOKS", "1"),
        ]);
        assert!(f.kairos);
        assert!(f.kairos_brief);
        assert!(f.kairos_channels);
        assert!(f.kairos_push_notification);
        assert!(f.kairos_github_webhooks);
        assert!(f.proactive);
    }

    #[test]
    fn is_enabled_query() {
        let f = flags(&[("FEATURE_KAIROS", "1"), ("FEATURE_KAIROS_BRIEF", "1")]);
        assert!(f.is_enabled(Feature::Kairos));
        assert!(f.is_enabled(Feature::KairosBrief));
        assert!(!f.is_enabled(Feature::KairosChannels));
        assert!(f.is_enabled(Feature::Proactive));
    }

    #[test]
    fn false_values_are_not_enabled() {
        let f = flags(&[("FEATURE_KAIROS", "0"), ("FEATURE_PROACTIVE", "false")]);
        assert!(!f.kairos);
        assert!(!f.proactive);
    }

    #[test]
    fn trimmed_and_case_insensitive() {
        let f = flags(&[
            ("FEATURE_KAIROS", " True "),
            ("FEATURE_KAIROS_BRIEF", " 1 "),
        ]);
        assert!(f.kairos);
        assert!(f.kairos_brief);
    }

    #[test]
    fn team_memory_standalone() {
        let f = flags(&[("FEATURE_TEAMMEM", "1")]);
        assert!(f.team_memory);
        assert!(!f.kairos, "team_memory should not imply kairos");
    }

    #[test]
    fn team_memory_default_off() {
        let f = flags(&[]);
        assert!(!f.team_memory);
    }

    #[test]
    fn subagent_dashboard_standalone() {
        let f = flags(&[("FEATURE_SUBAGENT_DASHBOARD", "1")]);
        assert!(f.subagent_dashboard);
        assert!(!f.kairos);
    }
}

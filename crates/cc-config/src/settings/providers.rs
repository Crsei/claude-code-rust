use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::raw::RawSettings;

pub const API_PROVIDER_ANTHROPIC: &str = "anthropic";
pub const API_PROVIDER_OPENAI_CODEX: &str = "openai-codex";
pub const API_PROVIDER_OPENAI: &str = "openai";
pub const VALID_API_PROVIDERS: &[&str] = &[
    API_PROVIDER_ANTHROPIC,
    API_PROVIDER_OPENAI_CODEX,
    API_PROVIDER_OPENAI,
];

pub fn normalize_api_provider(value: &str) -> Option<&'static str> {
    let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "anthropic" | "anthropic-method" | "anthropic_method" => Some(API_PROVIDER_ANTHROPIC),
        "openai-codex" | "openai_codex" | "codex" => Some(API_PROVIDER_OPENAI_CODEX),
        "openai" | "openai-api" | "openai_api" => Some(API_PROVIDER_OPENAI),
        _ => None,
    }
}

/// Provider/login profile stored under `authProfiles.<name>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProviderProfileSettings {
    pub backend: Option<String>,
    pub api_provider: Option<String>,
    pub model: Option<String>,
    pub available_models: Option<Vec<String>>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub auth_source: Option<Value>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl ProviderProfileSettings {
    pub fn is_effectively_empty(&self) -> bool {
        self.backend.is_none()
            && self.api_provider.is_none()
            && self.model.is_none()
            && self
                .available_models
                .as_ref()
                .is_none_or(|models| models.is_empty())
            && self.base_url.is_none()
            && self.api_key.is_none()
            && self.env.as_ref().is_none_or(HashMap::is_empty)
            && self.auth_source.is_none()
            && self.extra.is_empty()
    }
}

pub(crate) fn merge_provider_profile(
    base: &mut ProviderProfileSettings,
    over: ProviderProfileSettings,
) {
    if over.backend.is_some() {
        base.backend = over.backend;
    }
    if over.api_provider.is_some() {
        base.api_provider = over.api_provider;
    }
    if over.model.is_some() {
        base.model = over.model;
    }
    if over.available_models.is_some() {
        base.available_models = over.available_models;
    }
    if over.base_url.is_some() {
        base.base_url = over.base_url;
    }
    if over.api_key.is_some() {
        base.api_key = over.api_key;
    }
    if let Some(env) = over.env {
        let mut merged = base.env.take().unwrap_or_default();
        for (k, v) in env {
            merged.insert(k, v);
        }
        base.env = Some(merged);
    }
    if over.auth_source.is_some() {
        base.auth_source = over.auth_source;
    }
    for (k, v) in over.extra {
        base.extra.insert(k, v);
    }
}

pub fn upsert_auth_profile(
    raw: &mut RawSettings,
    name: &str,
    profile: ProviderProfileSettings,
    make_active: bool,
) {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return;
    }
    let profiles = raw.auth_profiles.get_or_insert_with(HashMap::new);
    profiles
        .entry(trimmed.to_string())
        .and_modify(|existing| merge_provider_profile(existing, profile.clone()))
        .or_insert(profile);
    if make_active {
        raw.active_auth_profile = Some(trimmed.to_string());
    }
}

pub fn auth_profile_name_for_provider(api_provider: &str) -> &'static str {
    match normalize_api_provider(api_provider).unwrap_or(api_provider) {
        API_PROVIDER_OPENAI_CODEX => "codex",
        API_PROVIDER_ANTHROPIC => "anthropic",
        API_PROVIDER_OPENAI => "openai",
        _ => "custom",
    }
}

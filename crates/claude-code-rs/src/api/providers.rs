//! Multi-provider registry and auto-detection.
//!
//! Supports 15+ LLM providers: Anthropic, OpenAI, Google Gemini,
//! and major Chinese LLM platforms (DeepSeek, Qwen, Zhipu, etc.).
//!
//! Each provider has a static `ProviderInfo` entry describing its
//! name, env var for API key, base URL, default model, and wire protocol.
//!
//! Reference: code-iris/crates/iris-llm

/// Wire protocol used by a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderProtocol {
    /// Native Anthropic Messages API (SSE with content_block events)
    Anthropic,
    /// OpenAI-compatible chat/completions API (SSE with choices/delta)
    OpenAiCompat,
    /// Google Gemini streamGenerateContent API
    Google,
}

/// Whether a provider returns native server-side streaming or cc-rust adapts
/// a non-streaming response into the unified `StreamEvent` shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingSupport {
    Native,
    Synthesized,
    None,
}

/// Current implementation status for a provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSupportStatus {
    Supported,
    Partial { reason: &'static str },
    Unsupported { reason: &'static str },
}

impl ProviderSupportStatus {
    pub fn is_usable(self) -> bool {
        matches!(self, Self::Supported | Self::Partial { .. })
    }

    pub fn reason(self) -> Option<&'static str> {
        match self {
            Self::Supported => None,
            Self::Partial { reason } => Some(reason),
            Self::Unsupported { reason } => Some(reason),
        }
    }
}

/// Provider capability matrix used by startup validation and diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub name: &'static str,
    pub auth_sources: &'static [&'static str],
    pub protocol: ProviderProtocol,
    pub streaming: StreamingSupport,
    pub tool_use: bool,
    pub thinking: bool,
    pub prompt_cache: bool,
    pub advisor: bool,
    pub status: ProviderSupportStatus,
}

impl ProviderCapabilities {
    pub fn is_usable(self) -> bool {
        self.status.is_usable()
    }
}

/// Static metadata about a supported LLM provider.
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// Short identifier, e.g. "deepseek"
    pub name: &'static str,
    /// Environment variable holding the API key, e.g. "DEEPSEEK_API_KEY"
    pub env_key: &'static str,
    /// Base URL for the API
    pub base_url: &'static str,
    /// Default model to use when none is specified
    pub default_model: &'static str,
    /// Human-readable label (bilingual for Chinese providers)
    #[allow(dead_code)]
    pub label: &'static str,
    /// Wire protocol (determines request/response format)
    pub protocol: ProviderProtocol,
}

const AUTH_ANTHROPIC: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "OAuth/keychain",
];
const AUTH_AZURE: &[&str] = &["AZURE_API_KEY", "AZURE_BASE_URL"];
const AUTH_OPENAI: &[&str] = &["OPENAI_API_KEY"];
const AUTH_OPENAI_CODEX: &[&str] = &["OPENAI_CODEX_AUTH_TOKEN", "~/.codex/auth.json"];
const AUTH_GOOGLE: &[&str] = &["GOOGLE_API_KEY"];
const AUTH_OPENAI_COMPAT: &[&str] = &["provider-specific API key env var"];
const AUTH_BEDROCK: &[&str] = &[
    "AWS_BEARER_TOKEN_BEDROCK",
    "AWS_ACCESS_KEY_ID + AWS_SECRET_ACCESS_KEY",
];
const AUTH_VERTEX: &[&str] = &[
    "CLAUDE_CODE_VERTEX_ACCESS_TOKEN",
    "GOOGLE_OAUTH_ACCESS_TOKEN",
    "gcloud application-default access token",
];
const AUTH_FOUNDRY: &[&str] = &["CLAUDE_CODE_USE_FOUNDRY"];

pub const BEDROCK_PARTIAL_REASON: &str =
    "Bedrock uses the non-streaming invoke endpoint and synthesizes StreamEvent output; native AWS EventStream support is still pending";
pub const VERTEX_PARTIAL_REASON: &str =
    "Vertex supports OAuth access tokens and gcloud ADC fallback; direct service-account JWT exchange is still pending";
pub const FOUNDRY_UNSUPPORTED_REASON: &str =
    "Foundry provider selection is known from the reference project, but cc-rust has no Foundry request/auth adapter yet";

/// All supported providers — ordered by detection priority.
///
/// The first provider with a set API key env var wins in `detect_provider()`.
pub static PROVIDERS: &[ProviderInfo] = &[
    // ── International ────────────────────────────────────────────
    ProviderInfo {
        name: "anthropic",
        env_key: "ANTHROPIC_API_KEY",
        base_url: "https://api.anthropic.com",
        default_model: "claude-sonnet-4-20250514",
        label: "Anthropic (Claude)",
        protocol: ProviderProtocol::Anthropic,
    },
    // Azure OpenAI — base_url is a placeholder; the real endpoint is read
    // from AZURE_BASE_URL at runtime (deployment-specific).
    ProviderInfo {
        name: "azure",
        env_key: "AZURE_API_KEY",
        base_url: "https://PLACEHOLDER.openai.azure.com/openai/v1",
        default_model: "gpt-4o",
        label: "Azure OpenAI",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "openai",
        env_key: "OPENAI_API_KEY",
        base_url: "https://api.openai.com/v1",
        default_model: "gpt-4o",
        label: "OpenAI (GPT)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    // ChatGPT OAuth token for Codex service (OpenAI Codex provider path).
    ProviderInfo {
        name: "openai-codex",
        env_key: "OPENAI_CODEX_AUTH_TOKEN",
        base_url: "https://chatgpt.com/backend-api",
        default_model: "gpt-5.4",
        label: "OpenAI Codex (ChatGPT OAuth)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "google",
        env_key: "GOOGLE_API_KEY",
        base_url: "https://generativelanguage.googleapis.com/v1beta",
        default_model: "gemini-2.0-flash",
        label: "Google (Gemini)",
        protocol: ProviderProtocol::Google,
    },
    ProviderInfo {
        name: "groq",
        env_key: "GROQ_API_KEY",
        base_url: "https://api.groq.com/openai/v1",
        default_model: "llama-3.3-70b-versatile",
        label: "Groq",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "openrouter",
        env_key: "OPENROUTER_API_KEY",
        base_url: "https://openrouter.ai/api/v1",
        default_model: "anthropic/claude-sonnet-4",
        label: "OpenRouter (多模型聚合)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    // ── China ────────────────────────────────────────────────────
    ProviderInfo {
        name: "deepseek",
        env_key: "DEEPSEEK_API_KEY",
        base_url: "https://api.deepseek.com/v1",
        default_model: "deepseek-chat",
        label: "DeepSeek (深度求索)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "zhipu",
        env_key: "ZHIPU_API_KEY",
        base_url: "https://open.bigmodel.cn/api/paas/v4",
        default_model: "glm-4-flash",
        label: "智谱 AI (GLM)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "qwen",
        env_key: "DASHSCOPE_API_KEY",
        base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
        default_model: "qwen-plus",
        label: "通义千问 (Qwen/百炼)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "moonshot",
        env_key: "MOONSHOT_API_KEY",
        base_url: "https://api.moonshot.cn/v1",
        default_model: "moonshot-v1-8k",
        label: "月之暗面 (Kimi)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "baichuan",
        env_key: "BAICHUAN_API_KEY",
        base_url: "https://api.baichuan-ai.com/v1",
        default_model: "Baichuan4-Air",
        label: "百川智能",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "minimax",
        env_key: "MINIMAX_API_KEY",
        base_url: "https://api.minimax.chat/v1",
        default_model: "MiniMax-Text-01",
        label: "MiniMax (稀宇)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "yi",
        env_key: "YI_API_KEY",
        base_url: "https://api.lingyiwanwu.com/v1",
        default_model: "yi-lightning",
        label: "零一万物 (Yi)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "siliconflow",
        env_key: "SILICONFLOW_API_KEY",
        base_url: "https://api.siliconflow.cn/v1",
        default_model: "deepseek-ai/DeepSeek-V3",
        label: "硅基流动 (SiliconFlow)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "stepfun",
        env_key: "STEPFUN_API_KEY",
        base_url: "https://api.stepfun.com/v1",
        default_model: "step-2-16k",
        label: "阶跃星辰 (StepFun)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
    ProviderInfo {
        name: "spark",
        env_key: "SPARK_API_KEY",
        base_url: "https://spark-api-open.xf-yun.com/v1",
        default_model: "generalv3.5",
        label: "讯飞星火 (Spark)",
        protocol: ProviderProtocol::OpenAiCompat,
    },
];

/// Return capabilities for a static `ProviderInfo` entry.
pub fn capabilities_for_provider_info(info: &ProviderInfo) -> ProviderCapabilities {
    let auth_sources = match info.name {
        "anthropic" => AUTH_ANTHROPIC,
        "azure" => AUTH_AZURE,
        "openai" => AUTH_OPENAI,
        "openai-codex" => AUTH_OPENAI_CODEX,
        "google" => AUTH_GOOGLE,
        _ => AUTH_OPENAI_COMPAT,
    };

    match info.protocol {
        ProviderProtocol::Anthropic => ProviderCapabilities {
            name: info.name,
            auth_sources,
            protocol: info.protocol,
            streaming: StreamingSupport::Native,
            tool_use: true,
            thinking: true,
            prompt_cache: true,
            advisor: true,
            status: ProviderSupportStatus::Supported,
        },
        ProviderProtocol::OpenAiCompat => ProviderCapabilities {
            name: info.name,
            auth_sources,
            protocol: info.protocol,
            streaming: StreamingSupport::Native,
            tool_use: true,
            thinking: false,
            prompt_cache: false,
            advisor: false,
            status: ProviderSupportStatus::Supported,
        },
        ProviderProtocol::Google => ProviderCapabilities {
            name: info.name,
            auth_sources,
            protocol: info.protocol,
            streaming: StreamingSupport::Native,
            tool_use: false,
            thinking: false,
            prompt_cache: false,
            advisor: false,
            status: ProviderSupportStatus::Supported,
        },
    }
}

/// Return capabilities by provider name, including env-flag providers that are
/// not part of the API-key auto-detection list.
pub fn capabilities_for_provider_name(name: &str) -> Option<ProviderCapabilities> {
    let name = name.to_ascii_lowercase();
    match name.as_str() {
        "bedrock" => Some(ProviderCapabilities {
            name: "bedrock",
            auth_sources: AUTH_BEDROCK,
            protocol: ProviderProtocol::Anthropic,
            streaming: StreamingSupport::Synthesized,
            tool_use: true,
            thinking: true,
            prompt_cache: false,
            advisor: true,
            status: ProviderSupportStatus::Partial {
                reason: BEDROCK_PARTIAL_REASON,
            },
        }),
        "vertex" => Some(ProviderCapabilities {
            name: "vertex",
            auth_sources: AUTH_VERTEX,
            protocol: ProviderProtocol::Anthropic,
            streaming: StreamingSupport::Native,
            tool_use: true,
            thinking: true,
            prompt_cache: false,
            advisor: true,
            status: ProviderSupportStatus::Partial {
                reason: VERTEX_PARTIAL_REASON,
            },
        }),
        "foundry" => Some(ProviderCapabilities {
            name: "foundry",
            auth_sources: AUTH_FOUNDRY,
            protocol: ProviderProtocol::Anthropic,
            streaming: StreamingSupport::None,
            tool_use: false,
            thinking: false,
            prompt_cache: false,
            advisor: false,
            status: ProviderSupportStatus::Unsupported {
                reason: FOUNDRY_UNSUPPORTED_REASON,
            },
        }),
        other => get_provider(other).map(capabilities_for_provider_info),
    }
}

/// Return the first provider that has an API key set in the environment.
pub fn detect_provider() -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| {
        std::env::var(p.env_key)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    })
}

/// Look up a provider by name (case-insensitive).
#[allow(dead_code)]
pub fn get_provider(name: &str) -> Option<&'static ProviderInfo> {
    let name_lower = name.to_lowercase();
    PROVIDERS.iter().find(|p| p.name == name_lower)
}

/// List all providers that currently have API keys set in the environment.
#[allow(dead_code)]
pub fn available_providers() -> Vec<&'static ProviderInfo> {
    PROVIDERS
        .iter()
        .filter(|p| {
            std::env::var(p.env_key)
                .map(|v| !v.is_empty())
                .unwrap_or(false)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_count() {
        assert_eq!(PROVIDERS.len(), 17);
    }

    #[test]
    fn test_get_provider_by_name() {
        let p = get_provider("deepseek").unwrap();
        assert_eq!(p.env_key, "DEEPSEEK_API_KEY");
        assert_eq!(p.protocol, ProviderProtocol::OpenAiCompat);
    }

    #[test]
    fn test_get_provider_case_insensitive() {
        let p = get_provider("DeepSeek").unwrap();
        assert_eq!(p.name, "deepseek");
    }

    #[test]
    fn test_get_provider_unknown() {
        assert!(get_provider("nonexistent").is_none());
    }

    #[test]
    fn test_anthropic_is_first() {
        assert_eq!(PROVIDERS[0].name, "anthropic");
        assert_eq!(PROVIDERS[0].protocol, ProviderProtocol::Anthropic);
    }

    #[test]
    fn test_google_protocol() {
        let p = get_provider("google").unwrap();
        assert_eq!(p.protocol, ProviderProtocol::Google);
    }

    #[test]
    fn test_openai_codex_protocol() {
        let p = get_provider("openai-codex").unwrap();
        assert_eq!(p.env_key, "OPENAI_CODEX_AUTH_TOKEN");
        assert_eq!(p.protocol, ProviderProtocol::OpenAiCompat);
    }

    #[test]
    fn test_capabilities_for_anthropic() {
        let caps = capabilities_for_provider_name("anthropic").unwrap();
        assert_eq!(caps.streaming, StreamingSupport::Native);
        assert!(caps.tool_use);
        assert!(caps.thinking);
        assert!(caps.prompt_cache);
        assert!(caps.advisor);
        assert!(caps.is_usable());
    }

    #[test]
    fn test_capabilities_for_openai_compat() {
        let caps = capabilities_for_provider_name("deepseek").unwrap();
        assert_eq!(caps.protocol, ProviderProtocol::OpenAiCompat);
        assert_eq!(caps.streaming, StreamingSupport::Native);
        assert!(caps.tool_use);
        assert!(!caps.thinking);
        assert!(!caps.prompt_cache);
        assert!(caps.is_usable());
    }

    #[test]
    fn test_capabilities_for_bedrock_are_partial() {
        let caps = capabilities_for_provider_name("bedrock").unwrap();
        assert_eq!(caps.streaming, StreamingSupport::Synthesized);
        assert!(caps.tool_use);
        assert!(caps.thinking);
        assert!(matches!(
            caps.status,
            ProviderSupportStatus::Partial { reason }
                if reason.contains("AWS EventStream")
        ));
        assert!(caps.is_usable());
    }

    #[test]
    fn test_capabilities_for_vertex_are_partial() {
        let caps = capabilities_for_provider_name("vertex").unwrap();
        assert_eq!(caps.streaming, StreamingSupport::Native);
        assert!(caps.tool_use);
        assert!(caps.thinking);
        assert!(matches!(
            caps.status,
            ProviderSupportStatus::Partial { reason }
                if reason.contains("service-account")
        ));
        assert!(caps.is_usable());
    }

    #[test]
    fn test_capabilities_for_foundry_are_unsupported() {
        let caps = capabilities_for_provider_name("foundry").unwrap();
        assert_eq!(caps.streaming, StreamingSupport::None);
        assert!(!caps.is_usable());
        assert!(matches!(
            caps.status,
            ProviderSupportStatus::Unsupported { reason }
                if reason.contains("no Foundry request/auth adapter")
        ));
    }

    #[test]
    fn test_all_chinese_providers_are_openai_compat() {
        let chinese = [
            "deepseek",
            "zhipu",
            "qwen",
            "moonshot",
            "baichuan",
            "minimax",
            "yi",
            "siliconflow",
            "stepfun",
            "spark",
        ];
        for name in chinese {
            let p = get_provider(name).expect(name);
            assert_eq!(
                p.protocol,
                ProviderProtocol::OpenAiCompat,
                "{} should be OpenAiCompat",
                name
            );
        }
    }
}

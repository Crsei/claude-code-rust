#![allow(unused)]
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
    pub label: &'static str,
    /// Wire protocol (determines request/response format)
    pub protocol: ProviderProtocol,
}

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

/// Return the first provider that has an API key set in the environment.
pub fn detect_provider() -> Option<&'static ProviderInfo> {
    PROVIDERS.iter().find(|p| {
        std::env::var(p.env_key)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    })
}

/// Look up a provider by name (case-insensitive).
pub fn get_provider(name: &str) -> Option<&'static ProviderInfo> {
    let name_lower = name.to_lowercase();
    PROVIDERS.iter().find(|p| p.name == name_lower)
}

/// List all providers that currently have API keys set in the environment.
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
// AWS Bedrock (interface only — not implemented)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct BedrockProvider {
    pub region: String,
    pub model_id: String,
}

#[allow(dead_code)]
impl BedrockProvider {
    pub fn new(region: &str, model_id: &str) -> Self {
        Self {
            region: region.to_string(),
            model_id: model_id.to_string(),
        }
    }

    pub fn endpoint_url(&self) -> String {
        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/invoke-with-response-stream",
            self.region, self.model_id
        )
    }

    pub async fn sign_request(
        &self,
        _body: &[u8],
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        anyhow::bail!("AWS Bedrock SigV4 signing is not implemented")
    }
}

// ---------------------------------------------------------------------------
// GCP Vertex AI (interface only — not implemented)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct VertexProvider {
    pub project_id: String,
    pub region: String,
}

#[allow(dead_code)]
impl VertexProvider {
    pub fn new(project_id: &str, region: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            region: region.to_string(),
        }
    }

    pub fn endpoint_url(&self, model: &str) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/anthropic/models/{}:streamRawPredict",
            self.region, self.project_id, self.region, model
        )
    }

    pub async fn get_access_token(&self) -> anyhow::Result<String> {
        anyhow::bail!("GCP Vertex AI authentication is not implemented")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_count() {
        assert_eq!(PROVIDERS.len(), 16);
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

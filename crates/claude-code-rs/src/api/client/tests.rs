use super::*;

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn anthropic_config() -> ApiClientConfig {
    ApiClientConfig {
        provider: ApiProvider::Anthropic {
            api_key: "sk-test-key-123".to_string(),
            base_url: None,
        },
        default_model: "claude-sonnet-4-20250514".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    }
}

fn anthropic_config_custom_url() -> ApiClientConfig {
    ApiClientConfig {
        provider: ApiProvider::Anthropic {
            api_key: "sk-test-key-456".to_string(),
            base_url: Some("https://custom.api.example.com".to_string()),
        },
        default_model: "claude-sonnet-4-20250514".to_string(),
        max_retries: 2,
        timeout_secs: 30,
    }
}

fn save_env(keys: &'static [&'static str]) -> Vec<(&'static str, Option<String>)> {
    keys.iter()
        .map(|key| (*key, std::env::var(key).ok()))
        .collect()
}

fn restore_env(saved: Vec<(&'static str, Option<String>)>) {
    for (key, value) in saved {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}

fn clear_env(keys: &[&str]) {
    for key in keys {
        std::env::remove_var(key);
    }
}

// -----------------------------------------------------------------------
// URL building
// -----------------------------------------------------------------------

#[test]
fn test_build_url_anthropic() {
    let client = ApiClient::new(anthropic_config());
    let url = client.build_url();
    assert_eq!(url, "https://api.anthropic.com/v1/messages");
}

#[test]
fn test_build_url_anthropic_custom_base() {
    let client = ApiClient::new(anthropic_config_custom_url());
    let url = client.build_url();
    assert_eq!(url, "https://custom.api.example.com/v1/messages");
}

#[test]
fn test_build_url_anthropic_trailing_slash() {
    let config = ApiClientConfig {
        provider: ApiProvider::Anthropic {
            api_key: "key".to_string(),
            base_url: Some("https://example.com/".to_string()),
        },
        default_model: "model".to_string(),
        max_retries: 1,
        timeout_secs: 30,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert_eq!(url, "https://example.com/v1/messages");
}

#[test]
fn test_build_url_bedrock_returns_aws_endpoint() {
    let config = ApiClientConfig {
        provider: ApiProvider::Bedrock {
            region: "us-east-1".to_string(),
            auth: crate::api::bedrock::BedrockAuth::BearerToken("dummy".to_string()),
            base_url_override: None,
        },
        default_model: "claude-sonnet-4-5-20250929".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert!(
        url.starts_with("https://bedrock-runtime.us-east-1.amazonaws.com/model/"),
        "unexpected URL: {url}"
    );
    assert!(url.ends_with("/invoke"), "unexpected URL: {url}");
    // Default model gets translated to its Bedrock ID.
    assert!(
        url.contains("us.anthropic.claude-sonnet-4-5-20250929-v1"),
        "URL missing translated Bedrock model: {url}"
    );
}

#[test]
fn test_build_url_bedrock_with_override() {
    let config = ApiClientConfig {
        provider: ApiProvider::Bedrock {
            region: "us-east-1".to_string(),
            auth: crate::api::bedrock::BedrockAuth::BearerToken("dummy".to_string()),
            base_url_override: Some("https://proxy.example.com".to_string()),
        },
        default_model: "claude-sonnet-4-5-20250929".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert!(
        url.starts_with("https://proxy.example.com/model/"),
        "override should be used, got: {url}"
    );
}

#[test]
fn test_build_url_vertex_returns_streamrawpredict() {
    let config = ApiClientConfig {
        provider: ApiProvider::Vertex {
            project_id: "my-project".to_string(),
            region: "us-east5".to_string(),
            access_token: crate::api::vertex::VertexAccessToken("dummy".to_string()),
        },
        default_model: "claude-sonnet-4-5-20250929".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert_eq!(
        url,
        "https://us-east5-aiplatform.googleapis.com/v1/projects/my-project/locations/us-east5/publishers/anthropic/models/claude-sonnet-4-5@20250929:streamRawPredict"
    );
}

#[test]
fn test_build_url_azure() {
    let config = ApiClientConfig {
        provider: ApiProvider::Azure {
            endpoint: "https://my-azure-endpoint.com".to_string(),
            api_key: "az-key".to_string(),
        },
        default_model: "model".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert_eq!(url, "https://my-azure-endpoint.com/v1/messages");
}

#[test]
fn test_build_url_openai_compat() {
    let config = ApiClientConfig {
        provider: ApiProvider::OpenAiCompat {
            name: "deepseek".to_string(),
            api_key: "sk-test".to_string(),
            base_url: "https://api.deepseek.com/v1".to_string(),
            default_model: "deepseek-chat".to_string(),
        },
        default_model: "deepseek-chat".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert_eq!(url, "https://api.deepseek.com/v1/chat/completions");
}

#[test]
fn test_build_url_openai_compat_trailing_slash() {
    let config = ApiClientConfig {
        provider: ApiProvider::OpenAiCompat {
            name: "openai".to_string(),
            api_key: "sk-test".to_string(),
            base_url: "https://api.openai.com/v1/".to_string(),
            default_model: "gpt-4o".to_string(),
        },
        default_model: "gpt-4o".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert_eq!(url, "https://api.openai.com/v1/chat/completions");
}

#[test]
fn test_build_url_openai_codex() {
    let config = ApiClientConfig {
        provider: ApiProvider::OpenAiCompat {
            name: OPENAI_CODEX_PROVIDER_NAME.to_string(),
            api_key: "token-test".to_string(),
            base_url: "https://chatgpt.com/backend-api/".to_string(),
            default_model: "gpt-5.4".to_string(),
        },
        default_model: "gpt-5.4".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let url = client.build_url();
    assert_eq!(url, "https://chatgpt.com/backend-api/conversation");
}

// -----------------------------------------------------------------------
// Header building
// -----------------------------------------------------------------------

#[test]
fn test_build_headers_has_required() {
    let client = ApiClient::new(anthropic_config());
    let headers = client.build_headers_map();

    assert_eq!(headers.get("content-type").unwrap(), "application/json");
    assert_eq!(headers.get("anthropic-version").unwrap(), "2023-06-01");
    assert_eq!(headers.get("x-api-key").unwrap(), "sk-test-key-123");
    assert!(headers
        .get("anthropic-beta")
        .unwrap()
        .contains("interleaved-thinking"));
    assert!(headers
        .get("anthropic-beta")
        .unwrap()
        .contains("prompt-caching"));
}

#[test]
fn test_build_headers_azure_has_api_key() {
    let config = ApiClientConfig {
        provider: ApiProvider::Azure {
            endpoint: "https://azure.example.com".to_string(),
            api_key: "az-secret".to_string(),
        },
        default_model: "model".to_string(),
        max_retries: 1,
        timeout_secs: 30,
    };
    let client = ApiClient::new(config);
    let headers = client.build_headers_map();
    assert_eq!(headers.get("x-api-key").unwrap(), "az-secret");
}

#[test]
fn test_build_headers_openai_compat_bearer() {
    let config = ApiClientConfig {
        provider: ApiProvider::OpenAiCompat {
            name: "openai".to_string(),
            api_key: "sk-my-key".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            default_model: "gpt-4o".to_string(),
        },
        default_model: "gpt-4o".to_string(),
        max_retries: 1,
        timeout_secs: 30,
    };
    let client = ApiClient::new(config);
    let headers = client.build_headers_map();
    assert_eq!(headers.get("Authorization").unwrap(), "Bearer sk-my-key");
    assert!(headers.get("x-api-key").is_none());
    assert!(headers.get("anthropic-version").is_none());
}

#[test]
fn test_build_headers_google_no_auth_header() {
    let config = ApiClientConfig {
        provider: ApiProvider::Google {
            api_key: "AIza-test".to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        },
        default_model: "gemini-2.0-flash".to_string(),
        max_retries: 1,
        timeout_secs: 30,
    };
    let client = ApiClient::new(config);
    let headers = client.build_headers_map();
    assert_eq!(headers.get("content-type").unwrap(), "application/json");
    assert!(headers.get("x-api-key").is_none());
    assert!(headers.get("Authorization").is_none());
}

#[test]
fn test_build_headers_bedrock_no_api_key() {
    let config = ApiClientConfig {
        provider: ApiProvider::Bedrock {
            region: "us-east-1".to_string(),
            auth: crate::api::bedrock::BedrockAuth::BearerToken("dummy".to_string()),
            base_url_override: None,
        },
        default_model: "model".to_string(),
        max_retries: 1,
        timeout_secs: 30,
    };
    let client = ApiClient::new(config);
    let headers = client.build_headers_map();
    // Generic header map deliberately does not include Bedrock auth — the
    // Bedrock provider sets Bearer/SigV4 headers per-request in its stream
    // implementation.
    assert!(headers.get("x-api-key").is_none());
    assert!(headers.get("authorization").is_none());
    assert_eq!(headers.get("content-type").unwrap(), "application/json");
}

// -----------------------------------------------------------------------
// from_provider_info
// -----------------------------------------------------------------------

#[test]
fn test_from_provider_info_anthropic() {
    use crate::api::providers::get_provider;
    let info = get_provider("anthropic").unwrap();
    let client = ApiClient::from_provider_info(info, "sk-test");
    assert!(matches!(
        client.config().provider,
        ApiProvider::Anthropic { .. }
    ));
    assert_eq!(client.config().default_model, "claude-sonnet-4-20250514");
}

#[test]
fn test_from_provider_info_deepseek() {
    use crate::api::providers::get_provider;
    let info = get_provider("deepseek").unwrap();
    let client = ApiClient::from_provider_info(info, "sk-ds-key");
    match &client.config().provider {
        ApiProvider::OpenAiCompat { name, base_url, .. } => {
            assert_eq!(name, "deepseek");
            assert_eq!(base_url, "https://api.deepseek.com/v1");
        }
        _ => panic!("expected OpenAiCompat"),
    }
}

#[test]
fn test_from_provider_info_google() {
    use crate::api::providers::get_provider;
    let info = get_provider("google").unwrap();
    let client = ApiClient::from_provider_info(info, "AIza-test");
    assert!(matches!(
        client.config().provider,
        ApiProvider::Google { .. }
    ));
    assert_eq!(client.config().default_model, "gemini-2.0-flash");
}

// -----------------------------------------------------------------------
// from_env / from_auth
// -----------------------------------------------------------------------

#[test]
fn test_from_env_with_anthropic_key() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    let saved_flags = save_env(&["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"]);
    clear_env(&["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"]);

    // Temporarily set the env var for this test
    let key = "sk-ant-api03-test-from-env-key";
    std::env::set_var("ANTHROPIC_API_KEY", key);

    let client = ApiClient::from_env();
    assert!(
        client.is_some(),
        "from_env should return Some when ANTHROPIC_API_KEY is set"
    );

    let client = client.unwrap();
    match &client.config().provider {
        ApiProvider::Anthropic { api_key, .. } => {
            assert_eq!(api_key, key);
        }
        other => panic!("expected Anthropic provider, got {:?}", other),
    }

    // Clean up
    std::env::remove_var("ANTHROPIC_API_KEY");
    restore_env(saved_flags);
}

#[test]
fn test_from_env_no_keys() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    let saved_flags = save_env(&["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"]);
    clear_env(&["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"]);

    // Save and clear all provider keys
    let saved: Vec<_> = crate::api::providers::PROVIDERS
        .iter()
        .filter_map(|p| std::env::var(p.env_key).ok().map(|v| (p.env_key, v)))
        .collect();
    for p in crate::api::providers::PROVIDERS {
        std::env::remove_var(p.env_key);
    }

    let client = ApiClient::from_env();
    assert!(
        client.is_none(),
        "from_env should return None when no provider key is set"
    );

    // Restore
    for (key, val) in saved {
        std::env::set_var(key, val);
    }
    restore_env(saved_flags);
}

#[test]
fn test_from_auth_with_env() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    let saved_flags = save_env(&["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"]);
    clear_env(&["CLAUDE_CODE_USE_BEDROCK", "CLAUDE_CODE_USE_VERTEX"]);

    let key = "sk-ant-api03-test-from-auth-key";
    std::env::set_var("ANTHROPIC_API_KEY", key);

    let client = ApiClient::from_auth();
    assert!(client.is_some(), "from_auth should find the env var");

    // Clean up
    std::env::remove_var("ANTHROPIC_API_KEY");
    restore_env(saved_flags);
}

#[test]
fn test_from_codex_auth_with_env() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    std::env::set_var(OPENAI_CODEX_TOKEN_ENV, "codex-token-test");
    std::env::set_var(OPENAI_CODEX_BASE_URL_ENV, "https://example.com/codex/");
    std::env::set_var(OPENAI_CODEX_MODEL_ENV, "gpt-5.3-codex-spark");

    let client = ApiClient::from_codex_auth().expect("from_codex_auth should return Some");
    match &client.config().provider {
        ApiProvider::OpenAiCompat {
            name,
            api_key,
            base_url,
            default_model,
        } => {
            assert_eq!(name, OPENAI_CODEX_PROVIDER_NAME);
            assert_eq!(api_key, "codex-token-test");
            assert_eq!(base_url, "https://example.com/codex");
            assert_eq!(default_model, "gpt-5.3-codex-spark");
        }
        other => panic!("expected OpenAiCompat provider, got {:?}", other),
    }
    assert_eq!(client.config().default_model, "gpt-5.3-codex-spark");

    std::env::remove_var(OPENAI_CODEX_TOKEN_ENV);
    std::env::remove_var(OPENAI_CODEX_BASE_URL_ENV);
    std::env::remove_var(OPENAI_CODEX_MODEL_ENV);
}

#[test]
fn test_from_env_prefers_bedrock_when_flag_set() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    let saved_extra = save_env(&[
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "AWS_BEARER_TOKEN_BEDROCK",
        "AWS_REGION",
    ]);
    // Save + clear all provider keys so Anthropic-API-key detection doesn't shadow.
    let saved_keys: Vec<_> = crate::api::providers::PROVIDERS
        .iter()
        .filter_map(|p| std::env::var(p.env_key).ok().map(|v| (p.env_key, v)))
        .collect();
    for p in crate::api::providers::PROVIDERS {
        std::env::remove_var(p.env_key);
    }

    std::env::set_var("CLAUDE_CODE_USE_BEDROCK", "1");
    std::env::set_var("AWS_BEARER_TOKEN_BEDROCK", "bedrock-123");
    std::env::set_var("AWS_REGION", "us-west-2");

    let client = ApiClient::from_env().expect("Bedrock flag should produce a client");
    match &client.config().provider {
        ApiProvider::Bedrock { region, .. } => assert_eq!(region, "us-west-2"),
        other => panic!("expected Bedrock provider, got {:?}", other),
    }

    for (k, v) in saved_keys {
        std::env::set_var(k, v);
    }
    restore_env(saved_extra);
}

#[test]
fn test_from_env_prefers_vertex_when_flag_set() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    let saved_extra = save_env(&[
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "CLAUDE_CODE_VERTEX_ACCESS_TOKEN",
        "CLOUD_ML_REGION",
    ]);
    let saved_keys: Vec<_> = crate::api::providers::PROVIDERS
        .iter()
        .filter_map(|p| std::env::var(p.env_key).ok().map(|v| (p.env_key, v)))
        .collect();
    for p in crate::api::providers::PROVIDERS {
        std::env::remove_var(p.env_key);
    }

    std::env::set_var("CLAUDE_CODE_USE_VERTEX", "true");
    std::env::set_var("ANTHROPIC_VERTEX_PROJECT_ID", "proj-42");
    std::env::set_var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN", "ya29.test");
    std::env::set_var("CLOUD_ML_REGION", "europe-west4");

    let client = ApiClient::from_env().expect("Vertex flag should produce a client");
    match &client.config().provider {
        ApiProvider::Vertex {
            project_id, region, ..
        } => {
            assert_eq!(project_id, "proj-42");
            assert_eq!(region, "europe-west4");
        }
        other => panic!("expected Vertex provider, got {:?}", other),
    }

    for (k, v) in saved_keys {
        std::env::set_var(k, v);
    }
    restore_env(saved_extra);
}

#[test]
fn test_from_bedrock_env_returns_none_without_auth() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    // Ensure neither auth method is available.
    let saved_bearer = std::env::var("AWS_BEARER_TOKEN_BEDROCK").ok();
    let saved_ak = std::env::var("AWS_ACCESS_KEY_ID").ok();
    let saved_sk = std::env::var("AWS_SECRET_ACCESS_KEY").ok();
    std::env::remove_var("AWS_BEARER_TOKEN_BEDROCK");
    std::env::remove_var("AWS_ACCESS_KEY_ID");
    std::env::remove_var("AWS_SECRET_ACCESS_KEY");

    assert!(
        ApiClient::from_bedrock_env_result().is_err(),
        "from_bedrock_env_result should reject missing AWS creds"
    );

    if let Some(v) = saved_bearer {
        std::env::set_var("AWS_BEARER_TOKEN_BEDROCK", v);
    }
    if let Some(v) = saved_ak {
        std::env::set_var("AWS_ACCESS_KEY_ID", v);
    }
    if let Some(v) = saved_sk {
        std::env::set_var("AWS_SECRET_ACCESS_KEY", v);
    }
}

#[test]
fn test_from_env_result_errors_for_explicit_bedrock_without_auth() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    let saved = save_env(&[
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "AWS_BEARER_TOKEN_BEDROCK",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
        "ANTHROPIC_API_KEY",
    ]);
    clear_env(&[
        "CLAUDE_CODE_USE_VERTEX",
        "AWS_BEARER_TOKEN_BEDROCK",
        "AWS_ACCESS_KEY_ID",
        "AWS_SECRET_ACCESS_KEY",
    ]);
    std::env::set_var("CLAUDE_CODE_USE_BEDROCK", "1");
    std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-api03-should-not-fallback");

    let err = match ApiClient::from_env_result() {
        Err(error) => error,
        Ok(_) => panic!("Bedrock config must fail early"),
    };
    let msg = err.to_string();
    assert!(msg.contains("CLAUDE_CODE_USE_BEDROCK"));
    assert!(msg.contains("AWS_BEARER_TOKEN_BEDROCK"));

    restore_env(saved);
}

#[test]
fn test_from_env_result_errors_for_explicit_vertex_without_project() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    let saved = save_env(&[
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_VERTEX",
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "GOOGLE_CLOUD_PROJECT",
        "GCLOUD_PROJECT",
        "CLAUDE_CODE_VERTEX_ACCESS_TOKEN",
        "ANTHROPIC_API_KEY",
    ]);
    clear_env(&[
        "CLAUDE_CODE_USE_BEDROCK",
        "ANTHROPIC_VERTEX_PROJECT_ID",
        "GOOGLE_CLOUD_PROJECT",
        "GCLOUD_PROJECT",
    ]);
    std::env::set_var("CLAUDE_CODE_USE_VERTEX", "1");
    std::env::set_var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN", "vertex-token");
    std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-api03-should-not-fallback");

    let err = match ApiClient::from_env_result() {
        Err(error) => error,
        Ok(_) => panic!("Vertex config must fail early"),
    };
    let msg = err.to_string();
    assert!(msg.contains("CLAUDE_CODE_USE_VERTEX"));
    assert!(msg.contains("ANTHROPIC_VERTEX_PROJECT_ID"));

    restore_env(saved);
}

#[test]
fn test_from_backend_codex_prefers_codex_auth() {
    let _env_lock = ENV_LOCK.lock().expect("env lock poisoned");
    std::env::set_var(OPENAI_CODEX_TOKEN_ENV, "codex-token-backend");
    std::env::set_var("ANTHROPIC_API_KEY", "sk-ant-api03-should-not-win");

    let client = ApiClient::from_backend(Some("codex")).expect("from_backend should return Some");
    match &client.config().provider {
        ApiProvider::OpenAiCompat { name, api_key, .. } => {
            assert_eq!(name, OPENAI_CODEX_PROVIDER_NAME);
            assert_eq!(api_key, "codex-token-backend");
        }
        other => panic!("expected OpenAiCompat provider, got {:?}", other),
    }

    std::env::remove_var(OPENAI_CODEX_TOKEN_ENV);
    std::env::remove_var("ANTHROPIC_API_KEY");
}

// -----------------------------------------------------------------------
// SSE line parsing
// -----------------------------------------------------------------------

#[test]
fn test_sse_line_parsing_message_start() {
    let sse_text = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":100,\"output_tokens\":0}}}\n\
\n";

    let events = parse_sse_text(sse_text).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        StreamEvent::MessageStart { usage } => {
            assert_eq!(usage.input_tokens, 100);
            assert_eq!(usage.output_tokens, 0);
        }
        other => panic!("expected MessageStart, got {:?}", other),
    }
}

#[test]
fn test_sse_line_parsing_content_block_start() {
    let sse_text = "\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n";

    let events = parse_sse_text(sse_text).unwrap();
    assert_eq!(events.len(), 1);
    match &events[0] {
        StreamEvent::ContentBlockStart {
            index,
            content_block: _content_block,
        } => {
            assert_eq!(*index, 0);
        }
        other => panic!("expected ContentBlockStart, got {:?}", other),
    }
}

#[test]
fn test_sse_line_parsing_multiple_events() {
    let sse_text = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":50,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":10}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

    let events = parse_sse_text(sse_text).unwrap();
    assert_eq!(events.len(), 6);

    assert!(matches!(events[0], StreamEvent::MessageStart { .. }));
    assert!(matches!(events[1], StreamEvent::ContentBlockStart { .. }));
    assert!(matches!(events[2], StreamEvent::ContentBlockDelta { .. }));
    assert!(matches!(events[3], StreamEvent::ContentBlockStop { .. }));
    assert!(matches!(events[4], StreamEvent::MessageDelta { .. }));
    assert!(matches!(events[5], StreamEvent::MessageStop));
}

#[test]
fn test_sse_line_parsing_ping_ignored() {
    let sse_text = "\
event: ping\n\
data: {}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

    let events = parse_sse_text(sse_text).unwrap();
    // ping should be ignored, only message_stop should come through
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], StreamEvent::MessageStop));
}

#[test]
fn test_sse_line_parsing_accumulator_integration() {
    let sse_text = "\
event: message_start\n\
data: {\"type\":\"message_start\",\"message\":{\"usage\":{\"input_tokens\":42,\"output_tokens\":0}}}\n\
\n\
event: content_block_start\n\
data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\
\n\
event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello, world!\"}}\n\
\n\
event: content_block_stop\n\
data: {\"type\":\"content_block_stop\",\"index\":0}\n\
\n\
event: message_delta\n\
data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\
\n\
event: message_stop\n\
data: {\"type\":\"message_stop\"}\n\
\n";

    let events = parse_sse_text(sse_text).unwrap();
    let mut acc = StreamAccumulator::new();
    for event in &events {
        acc.process_event(event);
    }

    let msg = acc.build("claude-sonnet-4-20250514");
    assert_eq!(msg.role, "assistant");
    assert_eq!(msg.content.len(), 1);
    assert_eq!(msg.stop_reason.as_deref(), Some("end_turn"));

    if let crate::types::message::ContentBlock::Text { text } = &msg.content[0] {
        assert_eq!(text, "Hello, world!");
    } else {
        panic!("expected Text content block");
    }

    assert_eq!(msg.usage.as_ref().unwrap().input_tokens, 42);
    assert_eq!(msg.usage.as_ref().unwrap().output_tokens, 5);
}

#[test]
fn test_sse_line_parsing_no_trailing_newline() {
    // SSE text without a trailing blank line should still parse
    let sse_text = "\
event: message_stop\n\
data: {\"type\":\"message_stop\"}";

    let events = parse_sse_text(sse_text).unwrap();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], StreamEvent::MessageStop));
}

#[test]
fn test_sse_line_parsing_empty_text() {
    let events = parse_sse_text("").unwrap();
    assert!(events.is_empty());
}

// -----------------------------------------------------------------------
// MessagesRequest serialization
// -----------------------------------------------------------------------

#[test]
fn test_messages_request_serialization() {
    let req = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![serde_json::json!({"role": "user", "content": "Hello"})],
        system: None,
        max_tokens: 1024,
        tools: None,
        stream: true,
        thinking: None,
        tool_choice: None,
        advisor_model: None,
    };

    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["model"], "claude-sonnet-4-20250514");
    assert_eq!(json["max_tokens"], 1024);
    assert_eq!(json["stream"], true);
    // thinking, tool_choice and advisor_model should be omitted when None
    assert!(json.get("thinking").is_none());
    assert!(json.get("tool_choice").is_none());
    assert!(json.get("advisor_model").is_none());
}

#[test]
fn test_messages_request_with_thinking() {
    let req = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![],
        system: Some(vec![
            serde_json::json!({"type": "text", "text": "You are helpful."}),
        ]),
        max_tokens: 4096,
        tools: None,
        stream: true,
        thinking: Some(serde_json::json!({"type": "enabled", "budget_tokens": 2048})),
        tool_choice: None,
        advisor_model: None,
    };

    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("thinking").is_some());
    assert_eq!(json["thinking"]["type"], "enabled");
    assert!(json.get("system").is_some());
}

#[test]
fn test_messages_request_advisor_model_serializes_when_set() {
    let req = MessagesRequest {
        model: "claude-sonnet-4-20250514".to_string(),
        messages: vec![],
        system: None,
        max_tokens: 1024,
        tools: None,
        stream: true,
        thinking: None,
        tool_choice: None,
        advisor_model: Some("claude-opus-4-20250514".to_string()),
    };

    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["advisor_model"], "claude-opus-4-20250514");
}

#[test]
fn test_provider_supports_advisor_matrix() {
    use crate::api::client::{provider_supports_advisor, ApiProvider};
    assert!(provider_supports_advisor(&ApiProvider::Anthropic {
        api_key: "k".into(),
        base_url: None,
    }));
    assert!(provider_supports_advisor(&ApiProvider::Azure {
        endpoint: "e".into(),
        api_key: "k".into(),
    }));
    assert!(!provider_supports_advisor(&ApiProvider::OpenAiCompat {
        name: "openai".into(),
        api_key: "k".into(),
        base_url: "u".into(),
        default_model: "m".into(),
    }));
    assert!(!provider_supports_advisor(&ApiProvider::Google {
        api_key: "k".into(),
        base_url: "u".into(),
    }));
}

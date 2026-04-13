use super::*;

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
#[should_panic(expected = "AWS Bedrock provider is not implemented")]
fn test_build_url_bedrock_not_implemented() {
    let config = ApiClientConfig {
        provider: ApiProvider::Bedrock {
            region: "us-east-1".to_string(),
            model_id: "anthropic.claude-sonnet-4-20250514-v1:0".to_string(),
        },
        default_model: "claude-sonnet-4-20250514".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let _ = client.build_url(); // should panic
}

#[test]
#[should_panic(expected = "GCP Vertex AI provider is not implemented")]
fn test_build_url_vertex_not_implemented() {
    let config = ApiClientConfig {
        provider: ApiProvider::Vertex {
            project_id: "my-project".to_string(),
            region: "us-central1".to_string(),
        },
        default_model: "claude-sonnet-4-20250514".to_string(),
        max_retries: 3,
        timeout_secs: 60,
    };
    let client = ApiClient::new(config);
    let _ = client.build_url(); // should panic
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
            model_id: "model-id".to_string(),
        },
        default_model: "model".to_string(),
        max_retries: 1,
        timeout_secs: 30,
    };
    let client = ApiClient::new(config);
    let headers = client.build_headers_map();
    assert!(headers.get("x-api-key").is_none());
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
}

#[test]
fn test_from_env_no_keys() {
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
}

#[test]
fn test_from_auth_with_env() {
    let key = "sk-ant-api03-test-from-auth-key";
    std::env::set_var("ANTHROPIC_API_KEY", key);

    let client = ApiClient::from_auth();
    assert!(client.is_some(), "from_auth should find the env var");

    // Clean up
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
    };

    let json = serde_json::to_value(&req).unwrap();
    assert_eq!(json["model"], "claude-sonnet-4-20250514");
    assert_eq!(json["max_tokens"], 1024);
    assert_eq!(json["stream"], true);
    // thinking and tool_choice should be omitted when None
    assert!(json.get("thinking").is_none());
    assert!(json.get("tool_choice").is_none());
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
    };

    let json = serde_json::to_value(&req).unwrap();
    assert!(json.get("thinking").is_some());
    assert_eq!(json["thinking"]["type"], "enabled");
    assert!(json.get("system").is_some());
}

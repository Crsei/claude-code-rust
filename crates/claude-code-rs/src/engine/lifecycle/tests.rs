#[cfg(test)]
mod tests {
    use crate::engine::lifecycle::*;
    use crate::engine::sdk_types::*;
    use crate::types::config::{QueryEngineConfig, QuerySource};
    use crate::types::message::Usage;

    fn make_config() -> QueryEngineConfig {
        QueryEngineConfig {
            cwd: "/tmp".to_string(),
            tools: vec![],
            custom_system_prompt: None,
            append_system_prompt: None,
            user_specified_model: None,
            fallback_model: None,
            max_turns: None,
            max_budget_usd: None,
            task_budget: None,
            verbose: false,
            initial_messages: None,
            commands: vec![],
            thinking_config: None,
            json_schema: None,
            replay_user_messages: false,
            persist_session: false,
            resolved_model: None,
            auto_save_session: false,
            agent_context: None,
        }
    }

    #[test]
    fn test_query_engine_creation() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.messages().len(), 0);
        assert_eq!(engine.total_turn_count(), 0);
        assert!(engine.usage().total_cost_usd == 0.0);
        assert!(!engine.session_id.as_str().is_empty());
    }

    #[test]
    fn test_query_engine_abort() {
        let engine = QueryEngine::new(make_config());
        assert!(!engine.is_aborted());
        assert!(engine.abort_reason().is_none());

        engine.abort();
        assert!(engine.is_aborted());
        assert!(matches!(
            engine.abort_reason(),
            Some(AbortReason::UserAbort)
        ));

        engine.reset_abort();
        assert!(!engine.is_aborted());
        assert!(engine.abort_reason().is_none());
    }

    #[test]
    fn test_query_engine_app_state() {
        let engine = QueryEngine::new(make_config());
        let state = engine.app_state();
        assert!(!state.verbose);

        engine.update_app_state(|s| {
            s.verbose = true;
        });

        let state = engine.app_state();
        assert!(state.verbose);
    }

    #[test]
    fn test_query_engine_permission_denial() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.permission_denials().len(), 0);

        engine.record_permission_denial(PermissionDenial {
            tool_name: "Bash".to_string(),
            tool_use_id: "tu_1".to_string(),
            reason: "user denied".to_string(),
            timestamp: 0,
        });

        assert_eq!(engine.permission_denials().len(), 1);
        assert_eq!(engine.permission_denials()[0].tool_name, "Bash");
    }

    #[test]
    fn test_usage_tracking() {
        let mut usage = UsageTracking::default();
        let api_usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_input_tokens: 10,
            cache_creation_input_tokens: 5,
        };
        usage.add_usage(&api_usage, 0.001);
        assert_eq!(usage.total_input_tokens, 100);
        assert_eq!(usage.total_output_tokens, 50);
        assert_eq!(usage.total_cache_read_tokens, 10);
        assert_eq!(usage.total_cache_creation_tokens, 5);
        assert!((usage.total_cost_usd - 0.001).abs() < f64::EPSILON);
        assert_eq!(usage.api_call_count, 1);

        // Second call accumulates
        usage.add_usage(&api_usage, 0.002);
        assert_eq!(usage.total_input_tokens, 200);
        assert_eq!(usage.api_call_count, 2);
    }

    #[test]
    fn test_discovered_skill_names() {
        let engine = QueryEngine::new(make_config());
        assert!(engine.discovered_skill_names().is_empty());

        engine
            .state
            .write()
            .discovered_skill_names
            .insert("test_skill".to_string());
        assert_eq!(engine.discovered_skill_names().len(), 1);
    }

    #[test]
    fn test_loaded_nested_memory_paths() {
        let engine = QueryEngine::new(make_config());
        assert!(engine.loaded_nested_memory_paths().is_empty());
    }

    #[test]
    fn test_set_tools() {
        let engine = QueryEngine::new(make_config());
        assert_eq!(engine.state.read().tools.len(), 0);
    }

    #[tokio::test]
    async fn test_submit_local_command() {
        use futures::StreamExt;

        let mut engine = QueryEngine::new(make_config());
        engine.set_command_dispatcher(std::sync::Arc::new(
            crate::commands::DefaultCommandDispatcher::new(),
        ));
        let stream = engine.submit_message("/clear", QuerySource::Sdk);
        let mut stream = std::pin::pin!(stream);

        let mut items: Vec<SdkMessage> = Vec::new();
        while let Some(msg) = stream.next().await {
            items.push(msg);
        }

        // Should yield SystemInit + Result
        assert!(
            items.len() >= 2,
            "expected at least 2 items, got {}",
            items.len()
        );

        // First should be SystemInit
        assert!(
            matches!(items[0], SdkMessage::SystemInit(_)),
            "first item should be SystemInit"
        );

        // Last should be Result with success
        let last = items.last().unwrap();
        match last {
            SdkMessage::Result(ref result) => {
                assert_eq!(result.subtype, ResultSubtype::Success);
                assert!(!result.is_error);
                assert!(result.result.contains("clear"));
            }
            other => panic!("expected SdkMessage::Result, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_submit_message_yields_system_init() {
        use futures::StreamExt;

        let engine = QueryEngine::new(make_config());
        let stream = engine.submit_message("hello", QuerySource::ReplMainThread);
        let mut stream = std::pin::pin!(stream);

        // The first item should always be SystemInit
        if let Some(msg) = stream.next().await {
            match msg {
                SdkMessage::SystemInit(init) => {
                    assert_eq!(init.session_id, engine.session_id.to_string());
                    assert!(!init.model.is_empty());
                }
                other => panic!("expected SystemInit, got {:?}", other),
            }
        } else {
            panic!("stream was empty");
        }
    }
}

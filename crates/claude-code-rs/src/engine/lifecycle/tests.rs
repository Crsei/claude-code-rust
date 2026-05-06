#[cfg(test)]
mod tests {
    use crate::engine::lifecycle::*;
    use crate::engine::sdk_types::*;
    use crate::types::config::{AgentContext, QueryEngineConfig, QuerySource};
    use crate::types::message::{Message, MessageContent, Usage, UserMessage};
    use tempfile::tempdir;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

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
        assert_eq!(engine.current_session_id(), engine.session_id);
    }

    #[test]
    fn test_query_engine_inherits_agent_team_context() {
        let mut config = make_config();
        config.agent_context = Some(AgentContext {
            agent_id: "researcher@alpha".to_string(),
            query_tracking: crate::types::tool::QueryChainTracking {
                chain_id: "chain-1".to_string(),
                depth: 1,
            },
            langfuse_session_id: "session-1".to_string(),
            agent_type: Some("Explore".to_string()),
            team_context: Some(cc_types::teams::TeamContext {
                team_name: "alpha".to_string(),
                ..Default::default()
            }),
        });

        let engine = QueryEngine::new(config);
        let app_state = &engine.state.read().app_state;
        assert_eq!(
            app_state
                .team_context
                .as_ref()
                .map(|context| context.team_name.as_str()),
            Some("alpha")
        );
    }

    #[test]
    fn test_start_new_session_rotates_active_id_and_clears_runtime_state() {
        let engine = QueryEngine::new(make_config());
        let original = engine.current_session_id();

        let next = engine.start_new_session();

        assert_ne!(next, original);
        assert_eq!(engine.current_session_id(), next);
        assert!(engine.messages().is_empty());
        let usage = engine.usage();
        assert_eq!(usage.total_input_tokens, 0);
        assert_eq!(usage.total_output_tokens, 0);
        assert_eq!(usage.total_cost_usd, 0.0);
    }

    #[test]
    #[serial_test::serial]
    fn test_start_new_session_saves_previous_messages() {
        let home = tempdir().unwrap();
        let _guard = EnvGuard::set("CC_RUST_HOME", home.path());
        let workspace = home.path().join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();

        let mut config = make_config();
        config.cwd = workspace.to_string_lossy().to_string();
        config.auto_save_session = true;
        let engine = QueryEngine::new(config);
        let previous = engine.current_session_id();
        engine.replace_messages(vec![Message::User(UserMessage {
            uuid: uuid::Uuid::new_v4(),
            timestamp: 1,
            role: "user".into(),
            content: MessageContent::Text("old message".into()),
            is_meta: false,
            tool_use_result: None,
            source_tool_assistant_uuid: None,
        })]);

        let next = engine.start_new_session();

        assert_ne!(previous, next);
        assert!(engine.messages().is_empty());
        let saved = crate::session::storage::load_session(previous.as_str()).unwrap();
        assert_eq!(saved.len(), 1);
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
        let original_session = engine.current_session_id();
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
                assert_eq!(result.session_id, engine.current_session_id().to_string());
            }
            other => panic!("expected SdkMessage::Result, got {:?}", other),
        }
        assert_ne!(engine.current_session_id(), original_session);
        assert!(engine.messages().is_empty());
    }

    #[tokio::test]
    async fn test_submit_output_command_executes_handler() {
        use futures::StreamExt;

        let mut engine = QueryEngine::new(make_config());
        engine.set_command_dispatcher(std::sync::Arc::new(
            crate::commands::DefaultCommandDispatcher::new(),
        ));
        let stream = engine.submit_message("/help clear", QuerySource::Sdk);
        let mut stream = std::pin::pin!(stream);

        let mut items: Vec<SdkMessage> = Vec::new();
        while let Some(msg) = stream.next().await {
            items.push(msg);
        }

        let result = items
            .iter()
            .find_map(|item| {
                if let SdkMessage::Result(result) = item {
                    Some(result)
                } else {
                    None
                }
            })
            .expect("result message");

        assert_eq!(result.subtype, ResultSubtype::Success);
        assert!(!result.is_error);
        assert!(result.result.contains("/clear"));
        assert!(!result.result.contains("help clear"));
    }

    #[tokio::test]
    async fn test_submit_query_command_injects_handler_messages_before_model_call() {
        use futures::StreamExt;

        let mut engine = QueryEngine::new(make_config());
        engine.set_command_dispatcher(std::sync::Arc::new(
            crate::commands::DefaultCommandDispatcher::new(),
        ));
        let stream = engine.submit_message("/review 123", QuerySource::Sdk);
        let mut stream = std::pin::pin!(stream);

        let first = stream.next().await.expect("system init");
        assert!(matches!(first, SdkMessage::SystemInit(_)));

        let messages = engine.messages();
        let review_prompt = messages.iter().find_map(|message| {
            if let Message::User(user) = message {
                if let MessageContent::Text(text) = &user.content {
                    return Some(text.as_str());
                }
            }
            None
        });
        assert!(
            matches!(review_prompt, Some(text) if text.contains("Review pull request `123`")),
            "query command should inject a user prompt before the first model call"
        );
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

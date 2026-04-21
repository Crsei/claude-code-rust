/// Stop hooks — 检查模型响应是否应触发继续/停止
///
/// 对应 TypeScript: query.ts 中 stop hooks 逻辑段
///
/// 在模型无工具调用的最终回复后执行, 可以:
///   - 允许停止 (正常终止)
///   - 阻止停止 (注入继续消息, 触发新轮次)
///   - 报告阻塞错误 (终止并标记为 hook 问题)
use anyhow::Result;

use cc_types::hooks::{HookEventConfig, HookRunner};
use crate::types::message::{AssistantMessage, Message};

/// Stop hook 检查结果
#[derive(Debug, Clone)]
pub enum StopHookResult {
    /// 允许停止 — 模型可以正常终止
    AllowStop,
    /// 阻止停止 — 需要继续对话 (注入消息)
    PreventStop {
        /// 要注入到对话中的续写消息内容
        continuation_message: String,
    },
    /// 阻塞错误 — hook 自身执行失败
    BlockingError { error: String },
}

/// 运行 stop hooks
///
/// 在模型返回无工具调用的回复后调用.
/// 检查所有配置的 stop hooks 来决定是否真的终止.
///
/// # Arguments
/// * `assistant_message` - 模型的最终回复
/// * `messages` - 完整对话历史
/// * `stop_hook_active` - 上一轮 stop hook 是否已激活 (防止无限循环)
/// * `hook_configs` - 从 settings.json 加载的 Stop 事件 hook 配置
///
/// # Returns
/// * `StopHookResult` - 决定是否允许终止
pub async fn run_stop_hooks(
    runner: &dyn HookRunner,
    _assistant_message: &AssistantMessage,
    _messages: &[Message],
    stop_hook_active: Option<bool>,
    hook_configs: &[HookEventConfig],
) -> Result<StopHookResult> {
    // Prevent infinite loops: if stop hook already fired once, allow stop
    if stop_hook_active == Some(true) {
        return Ok(StopHookResult::AllowStop);
    }

    use cc_types::hooks::PostToolHookResult;

    match runner.run_stop_hooks(hook_configs).await {
        Ok(PostToolHookResult::Continue) => Ok(StopHookResult::AllowStop),
        Ok(PostToolHookResult::StopContinuation { message }) => Ok(StopHookResult::PreventStop {
            continuation_message: message,
        }),
        Err(e) => Ok(StopHookResult::BlockingError {
            error: e.to_string(),
        }),
    }
}

/// 判断 assistant 消息是否包含工具调用
#[allow(dead_code)]
pub fn has_tool_use(assistant_message: &AssistantMessage) -> bool {
    assistant_message
        .content
        .iter()
        .any(|block| matches!(block, crate::types::message::ContentBlock::ToolUse { .. }))
}

/// 从 assistant 消息中提取所有工具调用
pub fn extract_tool_uses(
    assistant_message: &AssistantMessage,
) -> Vec<(String, String, serde_json::Value)> {
    assistant_message
        .content
        .iter()
        .filter_map(|block| match block {
            crate::types::message::ContentBlock::ToolUse { id, name, input } => {
                Some((id.clone(), name.clone(), input.clone()))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_assistant_message(
        content: Vec<crate::types::message::ContentBlock>,
    ) -> AssistantMessage {
        AssistantMessage {
            uuid: Uuid::new_v4(),
            timestamp: 0,
            role: "assistant".to_string(),
            content,
            usage: None,
            stop_reason: Some("end_turn".to_string()),
            is_api_error_message: false,
            api_error: None,
            cost_usd: 0.0,
        }
    }

    #[test]
    fn test_has_tool_use_empty() {
        use crate::types::message::ContentBlock;
        let msg = make_assistant_message(vec![ContentBlock::Text {
            text: "Hello".to_string(),
        }]);
        assert!(!has_tool_use(&msg));
    }

    #[test]
    fn test_has_tool_use_with_tool() {
        use crate::types::message::ContentBlock;
        let msg = make_assistant_message(vec![
            ContentBlock::Text {
                text: "Let me check".to_string(),
            },
            ContentBlock::ToolUse {
                id: "tu_1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
            },
        ]);
        assert!(has_tool_use(&msg));
    }

    #[test]
    fn test_extract_tool_uses() {
        use crate::types::message::ContentBlock;
        let msg = make_assistant_message(vec![
            ContentBlock::Text {
                text: "Running".to_string(),
            },
            ContentBlock::ToolUse {
                id: "tu_1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
            },
            ContentBlock::ToolUse {
                id: "tu_2".to_string(),
                name: "Read".to_string(),
                input: serde_json::json!({"file_path": "/tmp/x"}),
            },
        ]);
        let uses = extract_tool_uses(&msg);
        assert_eq!(uses.len(), 2);
        assert_eq!(uses[0].1, "Bash");
        assert_eq!(uses[1].1, "Read");
    }

    #[tokio::test]
    async fn test_stop_hooks_allow_by_default() {
        let msg = make_assistant_message(vec![]);
        let runner = cc_types::hooks::NoopHookRunner;
        let result = run_stop_hooks(&runner, &msg, &[], None, &[])
            .await
            .unwrap();
        assert!(matches!(result, StopHookResult::AllowStop));
    }
}

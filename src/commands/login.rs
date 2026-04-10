//! `/login` command — authenticate with an LLM provider.
//!
//! Usage:
//!   /login status           — show current auth status
//!   /login sk-ant-api03-... — store API key to system keychain
//!   /login                  — show guidance on how to authenticate

use anyhow::Result;
use async_trait::async_trait;

use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth;

pub struct LoginHandler;

#[async_trait]
impl CommandHandler for LoginHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();

        // ── /login status ──────────────────────────────────────────
        if args == "status" {
            let current_auth = auth::resolve_auth();
            let status = match &current_auth {
                auth::AuthMethod::ApiKey(key) => {
                    let masked = mask_key(key);
                    let source = if std::env::var("ANTHROPIC_API_KEY")
                        .map(|v| !v.is_empty())
                        .unwrap_or(false)
                    {
                        "环境变量 ANTHROPIC_API_KEY"
                    } else {
                        "系统 Keychain"
                    };
                    format!("已认证: API Key {} (来源: {})", masked, source)
                }
                auth::AuthMethod::ExternalToken(_) => {
                    "已认证: 外部 Token (ANTHROPIC_AUTH_TOKEN)".to_string()
                }
                auth::AuthMethod::OAuthToken { method, .. } => {
                    format!("已认证: OAuth Token ({})", method)
                }
                auth::AuthMethod::None => "未认证".to_string(),
            };
            return Ok(CommandResult::Output(status));
        }

        // ── /login <api-key> ───────────────────────────────────────
        if !args.is_empty() {
            // Treat the argument as an API key
            if auth::api_key::validate_api_key(args) {
                // Store to keychain
                match auth::api_key::store_api_key(args) {
                    Ok(()) => {
                        let masked = mask_key(args);
                        return Ok(CommandResult::Output(format!(
                            "API Key {} 已存储到系统 Keychain。\n\
                             下次启动时将自动从 Keychain 读取认证信息。",
                            masked
                        )));
                    }
                    Err(e) => {
                        return Ok(CommandResult::Output(format!(
                            "存储 API Key 到 Keychain 失败: {}\n\n\
                             可改用环境变量:\n  \
                             export ANTHROPIC_API_KEY=\"{}\"",
                            e,
                            mask_key(args)
                        )));
                    }
                }
            } else {
                return Ok(CommandResult::Output(
                    "无效的 API Key 格式。\n\
                     Anthropic API Key 以 \"sk-ant-\" 开头且长度大于 20 字符。\n\n\
                     用法:\n  \
                     /login sk-ant-api03-...\n  \
                     /login status"
                        .to_string(),
                ));
            }
        }

        // ── /login (无参数) ────────────────────────────────────────
        let current_auth = auth::resolve_auth();
        if current_auth.is_authenticated() {
            let method = match &current_auth {
                auth::AuthMethod::ApiKey(key) => format!("API Key ({})", mask_key(key)),
                auth::AuthMethod::ExternalToken(_) => "External Auth Token".to_string(),
                auth::AuthMethod::OAuthToken { method, .. } => {
                    format!("OAuth Token ({})", method)
                }
                auth::AuthMethod::None => unreachable!(),
            };
            return Ok(CommandResult::Output(format!(
                "当前已认证: {}\n\
                 使用 /logout 退出后可重新登录，或 /login status 查看详情。",
                method
            )));
        }

        Ok(CommandResult::Output(
            "认证方式:\n\n\
             1. 直接提供 API Key (存入系统 Keychain):\n   \
                /login sk-ant-api03-...\n\n\
             2. 设置环境变量 (推荐，支持多提供商):\n   \
                export ANTHROPIC_API_KEY=\"sk-ant-...\"\n   \
                export OPENAI_API_KEY=\"sk-...\"\n   \
                export GOOGLE_API_KEY=\"AIza...\"\n\n\
             3. 设置外部 Auth Token:\n   \
                export ANTHROPIC_AUTH_TOKEN=\"your-token\"\n\n\
             查看当前状态: /login status"
                .to_string(),
        ))
    }
}

/// Mask an API key for display: show prefix and last 4 chars.
fn mask_key(key: &str) -> String {
    if key.len() > 12 {
        format!("{}...{}", &key[..7], &key[key.len() - 4..])
    } else {
        "sk-ant-****".to_string()
    }
}

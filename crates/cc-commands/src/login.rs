//! `/login` command - authenticate with an LLM provider.
//!
//! Usage:
//!   /login                  - interactive login (choose method)
//!   /login status           - show current auth status
//!   /login sk-...           - store Anthropic/OpenAI API key directly
//!   /login 1..7             - select login/provider method
//!   /login bedrock|vertex   - enable a cloud provider for this process

use anyhow::Result;
use async_trait::async_trait;
use cc_config::settings::{self, RawSettings};

use super::login_code;
use crate::{CommandContext, CommandHandler, CommandResult};
use cc_auth::{self as auth, oauth::OAuthMethod};

pub struct LoginHandler;

#[async_trait]
impl CommandHandler for LoginHandler {
    async fn execute(&self, args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();

        if args == "status" {
            return Ok(CommandResult::Output(auth_status_text()));
        }

        if args.starts_with("sk-ant-") {
            return Ok(CommandResult::Output(store_anthropic_api_key(args, ctx)));
        }

        if args.starts_with("sk-") {
            return Ok(CommandResult::Output(store_openai_api_key(args, ctx)));
        }

        if args.is_empty() {
            return Ok(CommandResult::Output(login_menu()));
        }

        let mut parts = args.split_whitespace();
        let head = parts.next().unwrap_or_default();
        let rest = parts.collect::<Vec<_>>().join(" ");

        match head {
            "anthropic_method" | "anthropic-method" | "anthropic" => {
                if rest.trim().is_empty() {
                    Ok(CommandResult::Output(anthropic_login_menu()))
                } else {
                    execute_anthropic_method(&rest, ctx)
                }
            }
            "openai_api" | "openai-api" | "openai" => {
                if rest.trim().is_empty() {
                    Ok(CommandResult::Output(openai_api_prompt()))
                } else {
                    Ok(CommandResult::Output(store_openai_api_key(&rest, ctx)))
                }
            }
            "openai_codex" | "openai-codex" => Ok(CommandResult::Output(start_codex_oauth(ctx))),
            "1" => Ok(CommandResult::Output(
                "Paste your Anthropic API key:\n  /login sk-ant-api03-...".to_string(),
            )),
            "2" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::ClaudeAi,
            ))),
            "3" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::Console,
            ))),
            "4" | "codex" => Ok(CommandResult::Output(start_codex_oauth(ctx))),
            "5" | "codex-cli" => Ok(CommandResult::Output(check_codex_cli(ctx))),
            "6" | "bedrock" | "aws" => Ok(CommandResult::Output(enable_bedrock_session())),
            "7" | "vertex" | "vertex-ai" | "gcp" => {
                Ok(CommandResult::Output(enable_vertex_session()))
            }
            "cloud" | "platform" | "platforms" => Ok(CommandResult::Output(cloud_setup_text())),
            _ => Ok(CommandResult::Output(format!(
                "Unknown option: \"{}\"\n\n{}",
                args,
                login_menu()
            ))),
        }
    }
}

fn login_menu() -> String {
    "Select login method:\n\
     \n  anthropic_method  Anthropic API Key / Claude.ai OAuth / Console OAuth\
     \n  openai_codex      OpenAI Codex OAuth / Codex CLI import\
     \n  openai_api        OpenAI API Key\
     \n\nCompatibility shortcuts: /login 1..7, /login codex, /login codex-cli, /login bedrock, /login vertex, /login cloud"
        .to_string()
}

fn anthropic_login_menu() -> String {
    "Anthropic login methods:\n\
     \n  [1] API Key (paste manually)\
     \n  [2] Claude.ai OAuth (Pro/Max subscription)\
     \n  [3] Console OAuth (API billing)\
     \n\nType /login 1, /login 2, /login 3, or paste an Anthropic key with /login sk-ant-api03-..."
        .to_string()
}

fn openai_api_prompt() -> String {
    "Paste your OpenAI API key:\n  /login openai_api sk-...".to_string()
}

fn execute_anthropic_method(args: &str, ctx: &mut CommandContext) -> Result<CommandResult> {
    match args.trim() {
        "1" | "api" | "api-key" | "api_key" => Ok(CommandResult::Output(
            "Paste your Anthropic API key:\n  /login sk-ant-api03-...".to_string(),
        )),
        "2" | "claude" | "claude-ai" | "claude_ai" => Ok(CommandResult::Output(
            login_code::start_pending(OAuthMethod::ClaudeAi),
        )),
        "3" | "console" => Ok(CommandResult::Output(login_code::start_pending(
            OAuthMethod::Console,
        ))),
        key if key.starts_with("sk-ant-") => {
            Ok(CommandResult::Output(store_anthropic_api_key(key, ctx)))
        }
        other => Ok(CommandResult::Output(format!(
            "Unknown Anthropic login option: \"{}\"\n\n{}",
            other,
            anthropic_login_menu()
        ))),
    }
}

fn auth_status_text() -> String {
    if let Some(cloud_status) = cloud_auth_status_text() {
        return cloud_status;
    }

    if let Some(openai_status) = openai_api_status_text() {
        return openai_status;
    }

    if let Some(codex_status) = codex_auth_status_text() {
        return codex_status;
    }

    let current = match auth::try_resolve_auth() {
        Ok(current) => current,
        Err(error) => return format!("Authentication error: {error}"),
    };
    match &current {
        auth::AuthMethod::ApiKey(key) => {
            let source = if std::env::var("ANTHROPIC_API_KEY")
                .map(|v| !v.is_empty())
                .unwrap_or(false)
            {
                "env ANTHROPIC_API_KEY"
            } else {
                "system keychain"
            };
            format!(
                "Authenticated: API Key {} (source: {})",
                mask_key(key),
                source
            )
        }
        auth::AuthMethod::ExternalToken(_) => {
            "Authenticated: External Token (ANTHROPIC_AUTH_TOKEN)".to_string()
        }
        auth::AuthMethod::OAuthToken { method, .. } => {
            format!("Authenticated: OAuth ({})", method)
        }
        auth::AuthMethod::None => "Not authenticated".to_string(),
    }
}

fn openai_api_status_text() -> Option<String> {
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.trim().is_empty() {
            return Some(format!(
                "Authenticated: OpenAI API Key {} (source: env OPENAI_API_KEY)",
                mask_key(key.trim())
            ));
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let provider = settings::load_effective(&cwd)
        .ok()
        .and_then(|loaded| loaded.effective.api_provider)
        .and_then(|value| settings::normalize_api_provider(&value).map(str::to_string));
    if provider.as_deref() != Some(settings::API_PROVIDER_OPENAI) {
        return None;
    }

    match auth::api_key::load_openai_api_key() {
        Ok(Some(key)) if auth::api_key::validate_openai_api_key(&key) => Some(format!(
            "Authenticated: OpenAI API Key {} (source: system keychain)",
            mask_key(key.trim())
        )),
        Ok(Some(_)) => Some("OpenAI API key in system keychain is invalid.".to_string()),
        Ok(None) => {
            Some("OpenAI API provider selected, but no OpenAI API key is stored.".to_string())
        }
        Err(error) => Some(format!("OpenAI API keychain is not usable: {error}")),
    }
}

fn cloud_auth_status_text() -> Option<String> {
    if cc_api::api::client::is_env_truthy("CLAUDE_CODE_USE_FOUNDRY") {
        return Some(foundry_status_text());
    }
    if cc_api::api::client::is_env_truthy("CLAUDE_CODE_USE_BEDROCK") {
        return Some(bedrock_status_text());
    }
    if cc_api::api::client::is_env_truthy("CLAUDE_CODE_USE_VERTEX") {
        return Some(vertex_status_text());
    }
    None
}

fn enable_bedrock_session() -> String {
    std::env::set_var("CLAUDE_CODE_USE_BEDROCK", "1");
    std::env::remove_var("CLAUDE_CODE_USE_VERTEX");
    std::env::remove_var("CLAUDE_CODE_USE_FOUNDRY");

    let mut lines = vec![
        "AWS Bedrock provider enabled for this cc-rust session.".to_string(),
        "Next non-command prompt will use Bedrock because API clients are rebuilt per turn."
            .to_string(),
        String::new(),
        bedrock_status_text(),
    ];
    if cc_api::api::bedrock::BedrockAuth::from_env().is_none() {
        lines.push(String::new());
        lines.push(bedrock_setup_text());
    } else {
        lines.push(String::new());
        lines.push(
            "For future sessions, set CLAUDE_CODE_USE_BEDROCK=1 before launching cc-rust."
                .to_string(),
        );
    }
    lines.join("\n")
}

fn enable_vertex_session() -> String {
    std::env::set_var("CLAUDE_CODE_USE_VERTEX", "1");
    std::env::remove_var("CLAUDE_CODE_USE_BEDROCK");
    std::env::remove_var("CLAUDE_CODE_USE_FOUNDRY");

    let mut lines = vec![
        "GCP Vertex AI provider enabled for this cc-rust session.".to_string(),
        "Next non-command prompt will use Vertex because API clients are rebuilt per turn."
            .to_string(),
        String::new(),
        vertex_status_text(),
    ];
    if cc_api::api::vertex::resolve_project_id().is_none()
        || cc_api::api::vertex::VertexAccessToken::from_env_or_gcloud().is_none()
    {
        lines.push(String::new());
        lines.push(vertex_setup_text());
    } else {
        lines.push(String::new());
        lines.push(
            "For future sessions, set CLAUDE_CODE_USE_VERTEX=1 before launching cc-rust."
                .to_string(),
        );
    }
    lines.join("\n")
}

fn bedrock_status_text() -> String {
    let region = cc_api::api::bedrock::resolve_region();
    let base_url = std::env::var("ANTHROPIC_BEDROCK_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let model = std::env::var("ANTHROPIC_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "claude-sonnet-4-5-20250929".to_string());

    let auth = match cc_api::api::bedrock::BedrockAuth::from_env() {
        Some(cc_api::api::bedrock::BedrockAuth::BearerToken(token)) => {
            format!("Bearer token {}", mask_secret(&token))
        }
        Some(cc_api::api::bedrock::BedrockAuth::AwsCredentials(creds)) => {
            let session = if creds.session_token.is_some() {
                " + AWS_SESSION_TOKEN"
            } else {
                ""
            };
            format!(
                "SigV4 credentials {}{}",
                mask_secret(&creds.access_key_id),
                session
            )
        }
        None => {
            "missing (set AWS_BEARER_TOKEN_BEDROCK or AWS_ACCESS_KEY_ID + AWS_SECRET_ACCESS_KEY)"
                .to_string()
        }
    };

    format!(
        "Provider: AWS Bedrock\n\
         Enabled: {}\n\
         Region: {}\n\
         Auth: {}\n\
         Model: {}\n\
         Base URL: {}",
        cc_api::api::client::is_env_truthy("CLAUDE_CODE_USE_BEDROCK"),
        region,
        auth,
        model,
        base_url.unwrap_or_else(|| "default bedrock-runtime endpoint".to_string())
    )
}

fn vertex_status_text() -> String {
    let default_region = cc_api::api::vertex::resolve_region();
    let model = std::env::var("ANTHROPIC_MODEL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "claude-sonnet-4-5-20250929".to_string());
    let region =
        cc_api::api::vertex::resolve_region_for_model_with_default(Some(&model), &default_region);
    let project_id = cc_api::api::vertex::resolve_project_id()
        .unwrap_or_else(|| "missing (set ANTHROPIC_VERTEX_PROJECT_ID)".to_string());
    let token_source = vertex_token_source();

    format!(
        "Provider: GCP Vertex AI\n\
         Enabled: {}\n\
         Project: {}\n\
         Region: {}\n\
         Auth: {}\n\
         Model: {}",
        cc_api::api::client::is_env_truthy("CLAUDE_CODE_USE_VERTEX"),
        project_id,
        region,
        token_source,
        model
    )
}

fn vertex_token_source() -> String {
    if std::env::var("CLAUDE_CODE_VERTEX_ACCESS_TOKEN")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        return "CLAUDE_CODE_VERTEX_ACCESS_TOKEN".to_string();
    }
    if std::env::var("GOOGLE_OAUTH_ACCESS_TOKEN")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        return "GOOGLE_OAUTH_ACCESS_TOKEN".to_string();
    }
    if cc_api::api::vertex::VertexAccessToken::from_env_or_gcloud().is_some() {
        return "gcloud application-default access token".to_string();
    }
    "missing (set CLAUDE_CODE_VERTEX_ACCESS_TOKEN or run gcloud auth application-default login)"
        .to_string()
}

fn foundry_status_text() -> String {
    let validation = cc_api::api::providers::validate_provider_name("azure-foundry");
    let diagnostic = validation
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.as_str())
        .unwrap_or(cc_api::api::providers::FOUNDRY_UNSUPPORTED_REASON);
    format!(
        "Provider: Microsoft Foundry\n\
         Enabled: true\n\
         Status: unsupported\n\
         Diagnostic: {}\n\
         Action: unset CLAUDE_CODE_USE_FOUNDRY or choose /login bedrock or /login vertex.",
        diagnostic
    )
}

fn codex_auth_status_text() -> Option<String> {
    if std::env::var("OPENAI_CODEX_AUTH_TOKEN")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
    {
        return Some("Authenticated: OpenAI Codex OAuth (env OPENAI_CODEX_AUTH_TOKEN)".to_string());
    }

    // Check cc-rust's own credentials.json
    if let Ok(Some(stored)) = auth::token::load_token() {
        let method = stored.oauth_method.as_deref().unwrap_or_default();
        if method.eq_ignore_ascii_case("openai_codex") {
            return if auth::token::is_token_expired(&stored) {
                Some("OpenAI Codex OAuth token is expired. Run /login 4 to refresh.".to_string())
            } else {
                Some("Authenticated: OpenAI Codex OAuth (stored credentials)".to_string())
            };
        }
    }

    // Check Codex CLI fallback
    match auth::codex_cli::read_codex_cli_credential() {
        Ok(Some(cred)) => {
            return if auth::codex_cli::is_credential_expired(&cred) {
                Some(
                    "OpenAI Codex OAuth (from Codex CLI) is expired. \
                     Run /login 5 to refresh or /login 4 for a fresh login."
                        .to_string(),
                )
            } else {
                Some(
                    "Authenticated: OpenAI Codex OAuth (from Codex CLI ~/.codex/auth.json)"
                        .to_string(),
                )
            };
        }
        Ok(None) => {}
        Err(error) => {
            return Some(format!("Codex CLI auth.json is not usable: {error}"));
        }
    }

    None
}

fn cloud_setup_text() -> String {
    format!(
        "{}\n\n{}\n\n{}",
        bedrock_setup_text(),
        vertex_setup_text(),
        foundry_setup_text()
    )
}

fn bedrock_setup_text() -> String {
    "AWS Bedrock setup:\n\
     1. Set CLAUDE_CODE_USE_BEDROCK=1.\n\
     2. Set AWS_REGION or AWS_DEFAULT_REGION (default: us-east-1).\n\
     3. Use one auth mode:\n\
        - AWS_BEARER_TOKEN_BEDROCK=<bedrock-api-key>\n\
        - AWS_ACCESS_KEY_ID + AWS_SECRET_ACCESS_KEY (+ AWS_SESSION_TOKEN if needed)\n\
     4. Optional: ANTHROPIC_MODEL and ANTHROPIC_BEDROCK_BASE_URL.\n\
     Current session shortcut: /login bedrock"
        .to_string()
}

fn vertex_setup_text() -> String {
    "GCP Vertex AI setup:\n\
     1. Set CLAUDE_CODE_USE_VERTEX=1.\n\
     2. Set ANTHROPIC_VERTEX_PROJECT_ID (or GOOGLE_CLOUD_PROJECT / GCLOUD_PROJECT).\n\
     3. Set CLOUD_ML_REGION (default: us-east5) or per-model VERTEX_REGION_* overrides.\n\
     4. Provide auth with CLAUDE_CODE_VERTEX_ACCESS_TOKEN, GOOGLE_OAUTH_ACCESS_TOKEN,\n\
        or `gcloud auth application-default login`.\n\
     5. Optional: ANTHROPIC_MODEL.\n\
     Current session shortcut: /login vertex"
        .to_string()
}

fn foundry_setup_text() -> String {
    let validation = cc_api::api::providers::validate_provider_name("azure-foundry");
    let diagnostic = validation
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.as_str())
        .unwrap_or(cc_api::api::providers::FOUNDRY_UNSUPPORTED_REASON);
    format!(
        "Microsoft Foundry setup:\n\
         Status: unsupported in this cc-rust build.\n\
         Diagnostic: {}\n\
         Do not set CLAUDE_CODE_USE_FOUNDRY for this release.",
        diagnostic
    )
}

fn start_codex_oauth(ctx: &mut CommandContext) -> String {
    let mut msg = login_code::start_pending(OAuthMethod::OpenAiCodex);
    if !msg.starts_with("Cannot start OAuth flow") {
        let provider_msg =
            persist_provider_selection(settings::API_PROVIDER_OPENAI_CODEX, Some("codex"), ctx);
        if let Some(provider_msg) = provider_msg {
            msg.push_str("\n\n");
            msg.push_str(&provider_msg);
        }
    }
    msg
}

fn check_codex_cli(ctx: &mut CommandContext) -> String {
    let cred = match auth::codex_cli::read_codex_cli_credential() {
        Ok(Some(c)) => c,
        Ok(None) => {
            // Distinguish: file doesn't exist vs. wrong auth_mode
            if auth::codex_cli::codex_cli_auth_path().is_none() {
                return "Codex CLI not found. Install Codex CLI and run \
                        'codex' to log in first."
                    .to_string();
            }
            return "Codex CLI auth.json found but not usable \
                    (auth_mode is not chatgpt or tokens are missing). \
                    Use /login 1 to paste your API key, or /login 4 for OAuth."
                .to_string();
        }
        Err(error) => {
            return format!("Codex CLI auth.json is not usable: {error}");
        }
    };

    if !auth::codex_cli::is_credential_expired(&cred) {
        let mut msg = "Codex CLI credentials detected and valid. \
                cc-rust will use them automatically."
            .to_string();
        if let Some(provider_msg) =
            persist_provider_selection(settings::API_PROVIDER_OPENAI_CODEX, Some("codex"), ctx)
        {
            msg.push_str("\n\n");
            msg.push_str(&provider_msg);
        }
        return msg;
    }

    // Expired — try to refresh now
    match auth::try_resolve_codex_auth_token() {
        Ok(Some(_)) => {
            let mut msg = "Codex CLI token was expired but has been refreshed successfully. \
                 cc-rust will use it automatically."
                .to_string();
            if let Some(provider_msg) =
                persist_provider_selection(settings::API_PROVIDER_OPENAI_CODEX, Some("codex"), ctx)
            {
                msg.push_str("\n\n");
                msg.push_str(&provider_msg);
            }
            msg
        }
        Ok(None) => "Codex CLI token is expired and refresh failed. \
             Run /login 4 for a fresh OAuth login, or re-login in Codex CLI."
            .to_string(),
        Err(error) => format!("Codex CLI token refresh failed: {error}"),
    }
}

fn store_anthropic_api_key(key: &str, ctx: &mut CommandContext) -> String {
    if !auth::api_key::validate_api_key(key) {
        return "Invalid API key format. Keys start with \"sk-ant-\" and are >20 chars.".into();
    }
    match auth::api_key::store_api_key(key) {
        Ok(()) => {
            let mut msg = format!("Anthropic API Key {} stored to keychain.", mask_key(key));
            if let Some(provider_msg) =
                persist_provider_selection(settings::API_PROVIDER_ANTHROPIC, Some("native"), ctx)
            {
                msg.push_str("\n\n");
                msg.push_str(&provider_msg);
            }
            msg
        }
        Err(e) => format!("Failed to store API key: {}", e),
    }
}

fn store_openai_api_key(key: &str, ctx: &mut CommandContext) -> String {
    if !auth::api_key::validate_openai_api_key(key) {
        return "Invalid OpenAI API key format. Paste the full key after /login openai_api.".into();
    }
    match auth::api_key::store_openai_api_key(key) {
        Ok(()) => {
            let trimmed = key.trim();
            let mut msg = format!("OpenAI API Key {} stored to keychain.", mask_key(trimmed));
            if let Some(provider_msg) =
                persist_provider_selection(settings::API_PROVIDER_OPENAI, Some("native"), ctx)
            {
                msg.push_str("\n\n");
                msg.push_str(&provider_msg);
            }
            msg
        }
        Err(e) => format!("Failed to store OpenAI API key: {}", e),
    }
}

fn persist_provider_selection(
    api_provider: &str,
    backend: Option<&str>,
    ctx: &mut CommandContext,
) -> Option<String> {
    let path = settings::user_settings_path();
    let mut raw = if path.exists() {
        match std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|txt| serde_json::from_str::<RawSettings>(&txt).map_err(anyhow::Error::from))
        {
            Ok(raw) => raw,
            Err(error) => {
                return Some(format!(
                    "Provider selected for this session, but user settings were not updated: {}",
                    error
                ));
            }
        }
    } else {
        RawSettings::default()
    };

    raw.api_provider = Some(api_provider.to_string());
    if let Some(backend) = backend {
        raw.backend = Some(backend.to_string());
        ctx.app_state.main_loop_backend = backend.to_string();
        ctx.app_state.settings.backend = Some(backend.to_string());
        ctx.app_state
            .settings
            .sources
            .insert("backend".to_string(), settings::SettingsSource::User);
    }
    ctx.app_state.settings.api_provider = Some(api_provider.to_string());
    ctx.app_state
        .settings
        .sources
        .insert("apiProvider".to_string(), settings::SettingsSource::User);

    match settings::write_user_settings(&raw) {
        Ok(path) => Some(format!(
            "Selected apiProvider={}{} (persisted to {}).",
            api_provider,
            backend
                .map(|value| format!(", backend={value}"))
                .unwrap_or_default(),
            path.display()
        )),
        Err(error) => Some(format!(
            "Provider selected for this session, but user settings were not updated: {}",
            error
        )),
    }
}

fn mask_secret(value: &str) -> String {
    if value.len() > 12 {
        format!("{}...{}", &value[..4], &value[value.len() - 4..])
    } else {
        "****".to_string()
    }
}

fn mask_key(key: &str) -> String {
    if key.len() > 12 {
        format!("{}...{}", &key[..7], &key[key.len() - 4..])
    } else {
        "****".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cc_bootstrap::SessionId;
    use cc_engine::types::app_state::AppState;
    use std::path::PathBuf;

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let previous = std::env::var(key).ok();
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
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

    fn test_ctx() -> CommandContext {
        CommandContext {
            messages: Vec::new(),
            cwd: PathBuf::from("/test"),
            app_state: AppState::default(),
            session_id: SessionId::from_string("test-session"),
        }
    }

    #[test]
    fn test_mask_key_long() {
        let key = "sk-ant-api03-abcdefghijklmnop";
        let masked = mask_key(key);
        assert!(masked.starts_with("sk-ant-"));
        assert!(masked.contains("..."));
        assert!(masked.ends_with(&key[key.len() - 4..]));
    }

    #[test]
    fn test_mask_key_short() {
        let masked = mask_key("short");
        assert_eq!(masked, "****");
    }

    #[test]
    fn test_login_menu_contains_options() {
        let menu = login_menu();
        assert!(menu.contains("anthropic_method"));
        assert!(menu.contains("openai_codex"));
        assert!(menu.contains("openai_api"));
        assert!(menu.contains("/login 1..7"));
        assert!(menu.contains("/login codex-cli"));
    }

    #[test]
    fn test_anthropic_menu_contains_legacy_options() {
        let menu = anthropic_login_menu();
        assert!(menu.contains("[1]"));
        assert!(menu.contains("[2]"));
        assert!(menu.contains("[3]"));
        assert!(menu.contains("sk-ant-api03"));
    }

    #[test]
    fn test_enable_bedrock_session_sets_flag_and_reports_status() {
        let _lock = ENV_LOCK.lock().expect("env lock poisoned");
        let _bedrock = EnvGuard::set("CLAUDE_CODE_USE_BEDROCK", None);
        let _vertex = EnvGuard::set("CLAUDE_CODE_USE_VERTEX", Some("1"));
        let _foundry = EnvGuard::set("CLAUDE_CODE_USE_FOUNDRY", Some("1"));
        let _bearer = EnvGuard::set("AWS_BEARER_TOKEN_BEDROCK", Some("bedrock-token-1234"));
        let _region = EnvGuard::set("AWS_REGION", Some("us-west-2"));

        let text = enable_bedrock_session();

        assert!(cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_BEDROCK"
        ));
        assert!(!cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_VERTEX"
        ));
        assert!(!cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_FOUNDRY"
        ));
        assert!(text.contains("AWS Bedrock provider enabled"));
        assert!(text.contains("Region: us-west-2"));
        assert!(text.contains("Bearer token"));
    }

    #[test]
    fn test_enable_vertex_session_sets_flag_and_reports_status() {
        let _lock = ENV_LOCK.lock().expect("env lock poisoned");
        let _bedrock = EnvGuard::set("CLAUDE_CODE_USE_BEDROCK", Some("1"));
        let _vertex = EnvGuard::set("CLAUDE_CODE_USE_VERTEX", None);
        let _foundry = EnvGuard::set("CLAUDE_CODE_USE_FOUNDRY", Some("1"));
        let _project = EnvGuard::set("ANTHROPIC_VERTEX_PROJECT_ID", Some("proj-123"));
        let _token = EnvGuard::set("CLAUDE_CODE_VERTEX_ACCESS_TOKEN", Some("vertex-token"));
        let _region = EnvGuard::set("CLOUD_ML_REGION", Some("europe-west4"));

        let text = enable_vertex_session();

        assert!(cc_api::api::client::is_env_truthy("CLAUDE_CODE_USE_VERTEX"));
        assert!(!cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_BEDROCK"
        ));
        assert!(!cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_FOUNDRY"
        ));
        assert!(text.contains("GCP Vertex AI provider enabled"));
        assert!(text.contains("Project: proj-123"));
        assert!(text.contains("Region: europe-west4"));
        assert!(text.contains("CLAUDE_CODE_VERTEX_ACCESS_TOKEN"));
    }

    #[test]
    fn test_foundry_status_surfaces_provider_validation_diagnostic() {
        let _lock = ENV_LOCK.lock().expect("env lock poisoned");
        let _foundry = EnvGuard::set("CLAUDE_CODE_USE_FOUNDRY", Some("1"));
        let _bedrock = EnvGuard::set("CLAUDE_CODE_USE_BEDROCK", None);
        let _vertex = EnvGuard::set("CLAUDE_CODE_USE_VERTEX", None);

        let text = auth_status_text();

        assert!(text.contains("Microsoft Foundry"));
        assert!(text.contains("unsupported"));
        assert!(text.contains("no Foundry request/auth adapter"));
    }

    #[test]
    fn test_check_codex_cli_no_file() {
        // Point CODEX_HOME at a nonexistent directory
        let dir = tempfile::TempDir::new().unwrap();
        let empty_sub = dir.path().join("empty");
        std::fs::create_dir_all(&empty_sub).unwrap();
        let prev = std::env::var("CODEX_HOME").ok();
        std::env::set_var("CODEX_HOME", empty_sub.to_str().unwrap());

        let mut ctx = test_ctx();
        let msg = check_codex_cli(&mut ctx);
        assert!(
            msg.contains("not found"),
            "expected 'not found' message, got: {}",
            msg
        );

        match prev {
            Some(v) => std::env::set_var("CODEX_HOME", v),
            None => std::env::remove_var("CODEX_HOME"),
        }
    }
}

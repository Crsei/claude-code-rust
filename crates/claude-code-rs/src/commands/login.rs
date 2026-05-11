//! `/login` command - authenticate with an LLM provider.
//!
//! Usage:
//!   /login                  - interactive login (choose method)
//!   /login status           - show current auth status
//!   /login sk-ant-...       - store API key directly
//!   /login 1..7             - select login/provider method
//!   /login bedrock|vertex   - enable a cloud provider for this process

use anyhow::Result;
use async_trait::async_trait;

use super::login_code;
use super::{CommandContext, CommandHandler, CommandResult};
use crate::auth;
use crate::auth::oauth::OAuthMethod;

pub struct LoginHandler;

#[async_trait]
impl CommandHandler for LoginHandler {
    async fn execute(&self, args: &str, _ctx: &mut CommandContext) -> Result<CommandResult> {
        let args = args.trim();

        if args == "status" {
            return Ok(CommandResult::Output(auth_status_text()));
        }

        if args.starts_with("sk-ant-") {
            return Ok(CommandResult::Output(store_api_key(args)));
        }

        if args.is_empty() {
            return Ok(CommandResult::Output(login_menu()));
        }

        match args {
            "1" => Ok(CommandResult::Output(
                "Paste your API key:\n  /login sk-ant-api03-...".to_string(),
            )),
            "2" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::ClaudeAi,
            ))),
            "3" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::Console,
            ))),
            "4" | "codex" => Ok(CommandResult::Output(login_code::start_pending(
                OAuthMethod::OpenAiCodex,
            ))),
            "5" | "codex-cli" => Ok(CommandResult::Output(check_codex_cli())),
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
     \n  [1] API Key (paste manually)\
     \n  [2] Claude.ai OAuth (Pro/Max subscription)\
     \n  [3] Console OAuth (API billing)\
     \n  [4] OpenAI Codex OAuth (ChatGPT subscription)\
     \n  [5] Import from Codex CLI (~/.codex/auth.json)\
     \n  [6] AWS Bedrock (session env provider)\
     \n  [7] GCP Vertex AI (session env provider)\
     \n\nType /login 1..7, /login bedrock, /login vertex, or /login cloud"
        .to_string()
}

fn auth_status_text() -> String {
    if let Some(cloud_status) = cloud_auth_status_text() {
        return cloud_status;
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

fn cloud_auth_status_text() -> Option<String> {
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
    format!("{}\n\n{}", bedrock_setup_text(), vertex_setup_text())
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

fn check_codex_cli() -> String {
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
        return "Codex CLI credentials detected and valid. \
                cc-rust will use them automatically."
            .to_string();
    }

    // Expired — try to refresh now
    match auth::try_resolve_codex_auth_token() {
        Ok(Some(_)) => "Codex CLI token was expired but has been refreshed successfully. \
             cc-rust will use it automatically."
            .to_string(),
        Ok(None) => "Codex CLI token is expired and refresh failed. \
             Run /login 4 for a fresh OAuth login, or re-login in Codex CLI."
            .to_string(),
        Err(error) => format!("Codex CLI token refresh failed: {error}"),
    }
}

fn store_api_key(key: &str) -> String {
    if !auth::api_key::validate_api_key(key) {
        return "Invalid API key format. Keys start with \"sk-ant-\" and are >20 chars.".into();
    }
    match auth::api_key::store_api_key(key) {
        Ok(()) => format!("API Key {} stored to keychain.", mask_key(key)),
        Err(e) => format!("Failed to store API key: {}", e),
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
        "sk-ant-****".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(masked, "sk-ant-****");
    }

    #[test]
    fn test_login_menu_contains_options() {
        let menu = login_menu();
        assert!(menu.contains("[1]"));
        assert!(menu.contains("[2]"));
        assert!(menu.contains("[3]"));
        assert!(menu.contains("[4]"));
        assert!(menu.contains("[5]"));
        assert!(menu.contains("[6]"));
        assert!(menu.contains("[7]"));
        assert!(menu.contains("API Key"));
        assert!(menu.contains("OAuth"));
        assert!(menu.contains("Codex CLI"));
        assert!(menu.contains("Bedrock"));
        assert!(menu.contains("Vertex"));
    }

    #[test]
    fn test_enable_bedrock_session_sets_flag_and_reports_status() {
        let _lock = ENV_LOCK.lock().expect("env lock poisoned");
        let _bedrock = EnvGuard::set("CLAUDE_CODE_USE_BEDROCK", None);
        let _vertex = EnvGuard::set("CLAUDE_CODE_USE_VERTEX", Some("1"));
        let _bearer = EnvGuard::set("AWS_BEARER_TOKEN_BEDROCK", Some("bedrock-token-1234"));
        let _region = EnvGuard::set("AWS_REGION", Some("us-west-2"));

        let text = enable_bedrock_session();

        assert!(cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_BEDROCK"
        ));
        assert!(!cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_VERTEX"
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
        let _project = EnvGuard::set("ANTHROPIC_VERTEX_PROJECT_ID", Some("proj-123"));
        let _token = EnvGuard::set("CLAUDE_CODE_VERTEX_ACCESS_TOKEN", Some("vertex-token"));
        let _region = EnvGuard::set("CLOUD_ML_REGION", Some("europe-west4"));

        let text = enable_vertex_session();

        assert!(cc_api::api::client::is_env_truthy("CLAUDE_CODE_USE_VERTEX"));
        assert!(!cc_api::api::client::is_env_truthy(
            "CLAUDE_CODE_USE_BEDROCK"
        ));
        assert!(text.contains("GCP Vertex AI provider enabled"));
        assert!(text.contains("Project: proj-123"));
        assert!(text.contains("Region: europe-west4"));
        assert!(text.contains("CLAUDE_CODE_VERTEX_ACCESS_TOKEN"));
    }

    #[test]
    fn test_check_codex_cli_no_file() {
        // Point CODEX_HOME at a nonexistent directory
        let dir = tempfile::TempDir::new().unwrap();
        let empty_sub = dir.path().join("empty");
        std::fs::create_dir_all(&empty_sub).unwrap();
        let prev = std::env::var("CODEX_HOME").ok();
        std::env::set_var("CODEX_HOME", empty_sub.to_str().unwrap());

        let msg = check_codex_cli();
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

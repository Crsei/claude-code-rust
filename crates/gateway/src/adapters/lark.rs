use super::{
    adapter_diagnostic, AdapterProvider, AdapterStatus, AdapterTestMessage, RemoteAdapter,
};
use crate::config::LarkAdapterConfig;
use crate::GatewayError;
use serde_json::Value;
use std::sync::Arc;

const LARK_PROVIDER: AdapterProvider = AdapterProvider::Lark;

#[derive(Clone)]
pub struct LarkAdapter {
    config: LarkAdapterConfig,
    transport: Arc<dyn LarkTransport>,
}

impl LarkAdapter {
    pub fn new(config: LarkAdapterConfig) -> Self {
        Self::with_transport(config, DisabledLarkTransport)
    }

    pub fn with_transport<T>(config: LarkAdapterConfig, transport: T) -> Self
    where
        T: LarkTransport + 'static,
    {
        Self {
            config,
            transport: Arc::new(transport),
        }
    }

    fn mode(&self) -> Result<LarkConnectionMode<'_>, Box<AdapterStatus>> {
        let webhook_url = self
            .config
            .outbound_webhook_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let Some(webhook_url) = webhook_url {
            return Ok(LarkConnectionMode::Webhook { webhook_url });
        }

        let app_id = self
            .config
            .app_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let app_secret = self
            .config
            .app_secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if let (Some(app_id), Some(app_secret)) = (app_id, app_secret) {
            return Ok(LarkConnectionMode::AppCredentials { app_id, app_secret });
        }

        Err(Box::new(AdapterStatus::blocked(
            LARK_PROVIDER,
            "lark_credentials_missing",
            "Lark adapter credentials are missing.",
            "Configure an outbound webhook URL or both Lark app_id and app_secret.",
        )))
    }

    fn target_allowed(&self, target: &str) -> bool {
        self.config.test_target_allowlist.is_empty()
            || self
                .config
                .test_target_allowlist
                .iter()
                .any(|allowed| allowed == target)
    }
}

impl RemoteAdapter for LarkAdapter {
    fn provider(&self) -> AdapterProvider {
        LARK_PROVIDER
    }

    fn status(&self) -> AdapterStatus {
        if !self.config.enabled {
            return AdapterStatus::unconfigured(LARK_PROVIDER, "Lark adapter is not enabled.");
        }

        match self.mode() {
            Ok(LarkConnectionMode::Webhook { .. }) => AdapterStatus::blocked(
                LARK_PROVIDER,
                "lark_not_connected",
                "Lark outbound webhook is configured but has not been checked.",
                "Run adapter connect to verify outbound-only Lark delivery.",
            ),
            Ok(LarkConnectionMode::AppCredentials { .. }) => AdapterStatus::blocked(
                LARK_PROVIDER,
                "lark_not_connected",
                "Lark app credentials are configured but have not been checked.",
                "Run adapter connect to verify outbound-only Lark delivery.",
            ),
            Err(status) => *status,
        }
    }

    fn connect(&self) -> Result<AdapterStatus, GatewayError> {
        match self.mode().map_err(|status| status_error(*status))? {
            LarkConnectionMode::Webhook { webhook_url } => {
                self.transport.probe_webhook(webhook_url)?;
            }
            LarkConnectionMode::AppCredentials { app_id, app_secret } => {
                let response = self.transport.tenant_access_token(app_id, app_secret)?;
                ensure_lark_ok("tenant_access_token", response)?;
            }
        }

        Ok(AdapterStatus::connected_outbound_only(
            LARK_PROVIDER,
            "Lark credentials are valid for outbound delivery; inbound event control is not enabled.",
        ))
    }

    fn test_message(&self, message: AdapterTestMessage) -> Result<AdapterStatus, GatewayError> {
        if message.target.trim().is_empty() {
            return Err(GatewayError::new(adapter_diagnostic(
                LARK_PROVIDER,
                "lark_target_missing",
                "Lark test-message target is missing.",
                "Provide a configured Lark target or webhook route name.",
            )));
        }
        if !self.target_allowed(&message.target) {
            return Err(GatewayError::new(adapter_diagnostic(
                LARK_PROVIDER,
                "lark_target_blocked",
                "Lark test-message target is not allowlisted.",
                "Add the target to the Lark adapter test allowlist.",
            )));
        }

        match self.mode().map_err(|status| status_error(*status))? {
            LarkConnectionMode::Webhook { webhook_url } => {
                self.transport.send_webhook(webhook_url, &message.text)?;
            }
            LarkConnectionMode::AppCredentials { app_id, app_secret } => {
                let token_response = self.transport.tenant_access_token(app_id, app_secret)?;
                let tenant_access_token = tenant_access_token(token_response)?;
                self.transport.send_app_message(
                    &tenant_access_token,
                    &message.target,
                    &message.text,
                )?;
            }
        }

        Ok(AdapterStatus::connected_outbound_only(
            LARK_PROVIDER,
            "Lark test message was accepted for outbound delivery.",
        ))
    }
}

pub trait LarkTransport: Send + Sync {
    fn tenant_access_token(&self, app_id: &str, app_secret: &str) -> Result<Value, GatewayError>;
    fn probe_webhook(&self, webhook_url: &str) -> Result<(), GatewayError>;
    fn send_webhook(&self, webhook_url: &str, text: &str) -> Result<(), GatewayError>;
    fn send_app_message(
        &self,
        tenant_access_token: &str,
        target: &str,
        text: &str,
    ) -> Result<(), GatewayError>;
}

struct DisabledLarkTransport;

impl LarkTransport for DisabledLarkTransport {
    fn tenant_access_token(&self, _app_id: &str, _app_secret: &str) -> Result<Value, GatewayError> {
        Err(transport_unavailable("tenant_access_token"))
    }

    fn probe_webhook(&self, _webhook_url: &str) -> Result<(), GatewayError> {
        Err(transport_unavailable("probe_webhook"))
    }

    fn send_webhook(&self, _webhook_url: &str, _text: &str) -> Result<(), GatewayError> {
        Err(transport_unavailable("send_webhook"))
    }

    fn send_app_message(
        &self,
        _tenant_access_token: &str,
        _target: &str,
        _text: &str,
    ) -> Result<(), GatewayError> {
        Err(transport_unavailable("send_app_message"))
    }
}

fn transport_unavailable(operation: &'static str) -> GatewayError {
    GatewayError::new(
        adapter_diagnostic(
            LARK_PROVIDER,
            "lark_transport_unavailable",
            "Lark HTTP transport is not wired in this gateway layer yet.",
            "Inject a Lark transport from the daemon/API layer before connecting.",
        )
        .with_context(format!("provider=lark, operation={operation}")),
    )
}

fn ensure_lark_ok(operation: &'static str, response: Value) -> Result<(), GatewayError> {
    let code = response
        .get("code")
        .and_then(Value::as_i64)
        .or_else(|| response.get("StatusCode").and_then(Value::as_i64))
        .unwrap_or(0);
    if code == 0 {
        return Ok(());
    }

    Err(GatewayError::new(
        lark_diagnostic(
            "lark_api_rejected",
            "Lark rejected the adapter request.",
            "Verify the Lark adapter credentials, app permissions, and target configuration.",
            operation,
        )
        .with_context(format!(
            "provider=lark, operation={operation}, lark_code={code}"
        )),
    ))
}

fn tenant_access_token(response: Value) -> Result<String, GatewayError> {
    ensure_lark_ok("tenant_access_token", response.clone())?;
    response
        .get("tenant_access_token")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| {
            GatewayError::new(lark_diagnostic(
                "lark_token_missing",
                "Lark did not return a tenant access token.",
                "Verify the app credentials and Lark tenant permissions.",
                "tenant_access_token",
            ))
        })
}

fn status_error(status: AdapterStatus) -> GatewayError {
    GatewayError::new(status.diagnostic.unwrap_or_else(|| {
        adapter_diagnostic(
            LARK_PROVIDER,
            "lark_adapter_blocked",
            status.message,
            "Check Lark adapter configuration.",
        )
    }))
}

fn lark_diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    action: impl Into<String>,
    operation: impl AsRef<str>,
) -> crate::GatewayDiagnostic {
    crate::GatewayDiagnostic::new(code, message, action)
        .with_context(format!("provider=lark, operation={}", operation.as_ref()))
}

enum LarkConnectionMode<'a> {
    Webhook {
        webhook_url: &'a str,
    },
    AppCredentials {
        app_id: &'a str,
        app_secret: &'a str,
    },
}

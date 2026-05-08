use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::{GatewayDiagnostic, GatewayError};

pub const DAEMON_TOKEN_HEADER: &str = "x-cc-rust-daemon-token";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayAuthMode {
    LoopbackDaemonToken,
}

pub trait GatewayAuthVerifier: Clone + Send + Sync + 'static {
    fn mode(&self) -> GatewayAuthMode;
    fn verify(&self, candidate: Option<&str>) -> Result<(), GatewayError>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AllowAllGatewayAuth;

impl GatewayAuthVerifier for AllowAllGatewayAuth {
    fn mode(&self) -> GatewayAuthMode {
        GatewayAuthMode::LoopbackDaemonToken
    }

    fn verify(&self, _candidate: Option<&str>) -> Result<(), GatewayError> {
        Ok(())
    }
}

pub fn extract_gateway_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(DAEMON_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .or_else(|| {
            headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
        })
}

pub fn missing_token_error() -> GatewayError {
    GatewayError::new(GatewayDiagnostic::new(
        "missing_control_token",
        "The remote-control gateway request is missing a daemon control token.",
        "Send x-cc-rust-daemon-token or Authorization: Bearer with a valid local daemon token.",
    ))
}

pub fn invalid_token_error() -> GatewayError {
    GatewayError::new(GatewayDiagnostic::new(
        "invalid_control_token",
        "The remote-control gateway request has an invalid daemon control token.",
        "Refresh the daemon status and retry with the current local control token.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderMap;

    #[test]
    fn extracts_daemon_header_before_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(DAEMON_TOKEN_HEADER, "daemon-token".parse().unwrap());
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer bearer-token".parse().unwrap(),
        );

        assert_eq!(extract_gateway_token(&headers), Some("daemon-token"));
    }

    #[test]
    fn extracts_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            "Bearer bearer-token".parse().unwrap(),
        );

        assert_eq!(extract_gateway_token(&headers), Some("bearer-token"));
    }

    #[test]
    fn auth_errors_have_stable_codes() {
        assert_eq!(
            missing_token_error().diagnostic().code,
            "missing_control_token"
        );
        assert_eq!(
            invalid_token_error().diagnostic().code,
            "invalid_control_token"
        );
    }
}

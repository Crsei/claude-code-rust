use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::{GatewayDiagnostic, GatewayError};

type HmacSha256 = Hmac<Sha256>;

pub const GITHUB_SIGNATURE_HEADER: &str = "x-hub-signature-256";
pub const SLACK_SIGNATURE_HEADER: &str = "x-slack-signature";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookRouteConfig {
    pub route_id: String,
    #[serde(default, skip_serializing)]
    pub secret: Option<String>,
    #[serde(default)]
    pub max_body_bytes: usize,
}

impl WebhookRouteConfig {
    pub fn new(route_id: impl Into<String>, secret: Option<String>) -> Self {
        Self {
            route_id: route_id.into(),
            secret,
            max_body_bytes: 256 * 1024,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WebhookVerifier;

impl WebhookVerifier {
    pub fn verify_github(
        route: &WebhookRouteConfig,
        headers: &HeaderMap,
        body: &[u8],
    ) -> Result<(), GatewayError> {
        verify_size(route, body)?;
        let secret = route_secret(route)?;
        let signature = headers
            .get(GITHUB_SIGNATURE_HEADER)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(missing_signature_error)?;
        let Some(hex_signature) = signature.strip_prefix("sha256=") else {
            return Err(bad_hmac_error());
        };
        verify_hex_hmac(secret.as_bytes(), body, hex_signature)
    }
}

fn verify_size(route: &WebhookRouteConfig, body: &[u8]) -> Result<(), GatewayError> {
    if body.len() <= route.max_body_bytes {
        Ok(())
    } else {
        Err(GatewayError::new(GatewayDiagnostic::new(
            "payload_too_large",
            "The remote-control gateway payload is too large.",
            "Reduce the webhook body size and retry.",
        )))
    }
}

fn route_secret(route: &WebhookRouteConfig) -> Result<&str, GatewayError> {
    route
        .secret
        .as_deref()
        .map(str::trim)
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| {
            GatewayError::new(GatewayDiagnostic::new(
                "webhook_disabled",
                "Webhook route is disabled because its secret is missing.",
                "Configure a per-route webhook secret before enabling this route.",
            ))
        })
}

fn verify_hex_hmac(secret: &[u8], body: &[u8], hex_signature: &str) -> Result<(), GatewayError> {
    let expected = hmac_sha256(secret, body)?;
    let provided = hex::decode(hex_signature).map_err(|_| bad_hmac_error())?;
    if constant_time_eq(&provided, &expected) {
        Ok(())
    } else {
        Err(bad_hmac_error())
    }
}

fn hmac_sha256(secret: &[u8], body: &[u8]) -> Result<Vec<u8>, GatewayError> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|_| {
        GatewayError::new(GatewayDiagnostic::new(
            "bad_hmac",
            "Webhook signature could not be verified.",
            "Check the webhook secret and signature header.",
        ))
    })?;
    mac.update(body);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

fn missing_signature_error() -> GatewayError {
    GatewayError::new(GatewayDiagnostic::new(
        "missing_signature",
        "Webhook request is missing its signature header.",
        "Send the provider HMAC signature header with the webhook request.",
    ))
}

fn bad_hmac_error() -> GatewayError {
    GatewayError::new(GatewayDiagnostic::new(
        "bad_hmac",
        "Webhook signature could not be verified.",
        "Check the webhook secret and signature header.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_header(secret: &str, body: &[u8]) -> String {
        format!(
            "sha256={}",
            hex::encode(hmac_sha256(secret.as_bytes(), body).unwrap())
        )
    }

    #[test]
    fn github_hmac_accepts_valid_signature() {
        let route = WebhookRouteConfig::new("github", Some("route-secret".to_string()));
        let body = br#"{"event":"push"}"#;
        let mut headers = HeaderMap::new();
        headers.insert(
            GITHUB_SIGNATURE_HEADER,
            signed_header("route-secret", body).parse().unwrap(),
        );

        WebhookVerifier::verify_github(&route, &headers, body).unwrap();
    }

    #[test]
    fn github_hmac_rejects_bad_signature_without_leaking_secret() {
        let route = WebhookRouteConfig::new("github", Some("route-secret".to_string()));
        let mut headers = HeaderMap::new();
        headers.insert(
            GITHUB_SIGNATURE_HEADER,
            "sha256=0000000000000000000000000000000000000000000000000000000000000000"
                .parse()
                .unwrap(),
        );

        let err = WebhookVerifier::verify_github(&route, &headers, b"body").unwrap_err();
        let json = serde_json::to_string(err.diagnostic()).unwrap();

        assert_eq!(err.diagnostic().code, "bad_hmac");
        assert!(!json.contains("route-secret"));
    }

    #[test]
    fn missing_secret_disables_route() {
        let route = WebhookRouteConfig::new("github", None);
        let err = WebhookVerifier::verify_github(&route, &HeaderMap::new(), b"body").unwrap_err();

        assert_eq!(err.diagnostic().code, "webhook_disabled");
    }

    #[test]
    fn oversized_body_returns_payload_too_large() {
        let route = WebhookRouteConfig {
            route_id: "github".to_string(),
            secret: Some("route-secret".to_string()),
            max_body_bytes: 2,
        };
        let err = WebhookVerifier::verify_github(&route, &HeaderMap::new(), b"body").unwrap_err();

        assert_eq!(err.diagnostic().code, "payload_too_large");
    }
}

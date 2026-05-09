use axum::http::HeaderMap;
use gateway::webhook::{
    WebhookRouteConfig, WebhookRouteOutcome, WebhookRouter, WebhookVerifier,
    GITHUB_SIGNATURE_HEADER,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

fn signed_header(secret: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
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
    let mut route = WebhookRouteConfig::github("github", Some("route-secret".to_string()));
    route.max_body_bytes = 2;
    let err = WebhookVerifier::verify_github(&route, &HeaderMap::new(), b"body").unwrap_err();

    assert_eq!(err.diagnostic().code, "payload_too_large");
}

#[test]
fn route_filters_unaccepted_events_without_rendering_prompt() {
    let route = WebhookRouteConfig::github("github", Some("route-secret".to_string()));
    let body = br#"{"action":"opened"}"#;
    let mut headers = HeaderMap::new();
    headers.insert(
        GITHUB_SIGNATURE_HEADER,
        signed_header("route-secret", body).parse().unwrap(),
    );
    headers.insert("x-github-event", "push".parse().unwrap());

    let outcome = WebhookRouter::handle(&route, &headers, body).unwrap();

    assert_eq!(
        outcome,
        WebhookRouteOutcome::Ignored {
            event: Some("push".to_string())
        }
    );
}

#[test]
fn route_renders_prompt_and_extracts_github_delivery_id() {
    let route = WebhookRouteConfig::github("github", Some("route-secret".to_string()));
    let body = br#"{"action":"opened","repository":{"full_name":"acme/demo"},"pull_request":{"number":7},"sender":{"login":"octocat"}}"#;
    let mut headers = HeaderMap::new();
    headers.insert(
        GITHUB_SIGNATURE_HEADER,
        signed_header("route-secret", body).parse().unwrap(),
    );
    headers.insert("x-github-event", "pull_request".parse().unwrap());
    headers.insert("x-github-delivery", "delivery-7".parse().unwrap());

    let outcome = WebhookRouter::handle(&route, &headers, body).unwrap();
    let WebhookRouteOutcome::Run { request, event } = outcome else {
        panic!("expected run outcome");
    };

    assert_eq!(event.as_deref(), Some("pull_request"));
    assert_eq!(request.idempotency_key.as_deref(), Some("delivery-7"));
    assert!(request.prompt.contains("acme/demo"));
    assert_eq!(request.source.client_id, "github");
    assert_eq!(request.source.user_id, "octocat");
    assert_eq!(request.source.thread_id, "pr-7");
}

#[test]
fn deliver_only_route_does_not_create_run_request() {
    let route =
        WebhookRouteConfig::github("github", Some("route-secret".to_string())).with_deliver_only(true);
    let body = br#"{"action":"opened","repository":{"full_name":"acme/demo"}}"#;
    let mut headers = HeaderMap::new();
    headers.insert(
        GITHUB_SIGNATURE_HEADER,
        signed_header("route-secret", body).parse().unwrap(),
    );
    headers.insert("x-github-event", "pull_request".parse().unwrap());

    let outcome = WebhookRouter::handle(&route, &headers, body).unwrap();

    assert!(matches!(outcome, WebhookRouteOutcome::DeliverOnly { .. }));
}

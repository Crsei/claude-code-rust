//! Webhook signature verification and payload parsing.

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Verify GitHub webhook signature (X-Hub-Signature-256).
///
/// GitHub sends a header `X-Hub-Signature-256: sha256=<hex>` on each webhook
/// delivery. This function recomputes the HMAC-SHA256 over the raw body using
/// the shared secret and compares it to the provided signature in constant time.
pub fn verify_github_signature(body: &[u8], signature: &str, secret: &str) -> bool {
    let Some(hex_sig) = signature.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let Ok(expected) = hex::decode(hex_sig) else {
        return false;
    };
    mac.verify_slice(&expected).is_ok()
}

/// Verify Slack webhook signature (X-Slack-Signature / X-Slack-Request-Timestamp).
///
/// Slack computes `v0=HMAC-SHA256(signing_secret, "v0:{timestamp}:{body}")` and
/// sends it as `X-Slack-Signature`. This function rebuilds the base string and
/// compares the result.
pub fn verify_slack_signature(
    body: &[u8],
    timestamp: &str,
    signature: &str,
    signing_secret: &str,
) -> bool {
    let sig_basestring = format!("v0:{}:{}", timestamp, String::from_utf8_lossy(body));
    let Ok(mut mac) = HmacSha256::new_from_slice(signing_secret.as_bytes()) else {
        return false;
    };
    mac.update(sig_basestring.as_bytes());
    let result = mac.finalize();
    let expected = format!("v0={}", hex::encode(result.into_bytes()));
    expected == signature
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- GitHub ----

    #[test]
    fn test_github_signature_valid() {
        let secret = "my-github-secret";
        let body = b"{ \"action\": \"opened\" }";

        // Compute the real HMAC so we have a known-good signature.
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let sig_hex = hex::encode(mac.finalize().into_bytes());
        let header = format!("sha256={sig_hex}");

        assert!(verify_github_signature(body, &header, secret));
    }

    #[test]
    fn test_github_signature_invalid() {
        let secret = "my-github-secret";
        let body = b"{ \"action\": \"opened\" }";
        let bad_header = "sha256=0000000000000000000000000000000000000000000000000000000000000000";

        assert!(!verify_github_signature(body, bad_header, secret));
    }

    #[test]
    fn test_github_signature_missing_prefix() {
        // No "sha256=" prefix should be rejected immediately.
        assert!(!verify_github_signature(b"body", "deadbeef", "secret"));
    }

    #[test]
    fn test_github_signature_invalid_hex() {
        // Non-hex characters after prefix should be rejected.
        assert!(!verify_github_signature(b"body", "sha256=ZZZZ", "secret"));
    }

    // ---- Slack ----

    #[test]
    fn test_slack_signature_valid() {
        let signing_secret = "my-slack-signing-secret";
        let timestamp = "1631234567";
        let body = b"token=abc123&event=url_verification";

        // Compute expected signature.
        let sig_basestring = format!(
            "v0:{}:{}",
            timestamp,
            String::from_utf8_lossy(body)
        );
        let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes()).unwrap();
        mac.update(sig_basestring.as_bytes());
        let expected = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

        assert!(verify_slack_signature(body, timestamp, &expected, signing_secret));
    }

    #[test]
    fn test_slack_signature_invalid() {
        let signing_secret = "my-slack-signing-secret";
        let timestamp = "1631234567";
        let body = b"token=abc123&event=url_verification";
        let bad_sig = "v0=0000000000000000000000000000000000000000000000000000000000000000";

        assert!(!verify_slack_signature(body, timestamp, bad_sig, signing_secret));
    }

    #[test]
    fn test_slack_signature_wrong_timestamp() {
        let signing_secret = "my-slack-signing-secret";
        let body = b"payload";

        // Compute with one timestamp, verify with a different one.
        let ts_sign = "1000000000";
        let ts_verify = "9999999999";

        let sig_basestring = format!("v0:{}:{}", ts_sign, String::from_utf8_lossy(body));
        let mut mac = HmacSha256::new_from_slice(signing_secret.as_bytes()).unwrap();
        mac.update(sig_basestring.as_bytes());
        let sig = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

        assert!(!verify_slack_signature(body, ts_verify, &sig, signing_secret));
    }
}

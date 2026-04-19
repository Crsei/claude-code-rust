//! Network policy — domain allowlist + `--no-network` gating.
//!
//! Shared by [`crate::tools::web_fetch`] and the shell sandboxes.

use super::errors::SandboxError;

/// Outcome of a network check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkDecision {
    Allowed,
    Denied(SandboxError),
}

/// Compiled network policy.
#[derive(Debug, Clone, Default)]
pub struct NetworkPolicy {
    /// If `true`, all outbound requests are blocked.
    pub disabled: bool,
    /// If non-empty, only hosts matching one of these patterns are allowed.
    ///
    /// Patterns:
    /// - Bare hostname → exact match
    /// - `*.example.com` → matches `x.example.com` and `example.com` itself
    /// - `example.com` → matches `example.com` AND its subdomains
    ///   (matches the Claude Code spec behavior)
    pub allowed_domains: Vec<String>,
}

impl NetworkPolicy {
    pub fn check_host(&self, host: &str) -> NetworkDecision {
        if self.disabled {
            return NetworkDecision::Denied(SandboxError::NetworkDisabled);
        }
        if self.allowed_domains.is_empty() {
            return NetworkDecision::Allowed;
        }
        let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
        for pattern in &self.allowed_domains {
            if matches_domain(&host, pattern) {
                return NetworkDecision::Allowed;
            }
        }
        NetworkDecision::Denied(SandboxError::DomainNotAllowed { host })
    }

    /// Convenience check for full URLs. Extracts the host and delegates.
    pub fn check_url(&self, url: &str) -> NetworkDecision {
        if self.disabled {
            return NetworkDecision::Denied(SandboxError::NetworkDisabled);
        }
        match ::url::Url::parse(url) {
            Ok(u) => match u.host_str() {
                Some(h) => self.check_host(h),
                None => NetworkDecision::Denied(SandboxError::Policy {
                    message: format!("URL has no host: {}", url),
                }),
            },
            Err(_) => NetworkDecision::Denied(SandboxError::Policy {
                message: format!("invalid URL: {}", url),
            }),
        }
    }
}

fn matches_domain(host: &str, pattern: &str) -> bool {
    let pat = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
    if pat.is_empty() {
        return false;
    }
    if let Some(rest) = pat.strip_prefix("*.") {
        // `*.example.com` → matches `foo.example.com` and `example.com`
        return host == rest || host.ends_with(&format!(".{}", rest));
    }
    // Bare `example.com` → matches itself + subdomains
    host == pat || host.ends_with(&format!(".{}", pat))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_denies_everything() {
        let p = NetworkPolicy {
            disabled: true,
            ..Default::default()
        };
        assert!(matches!(
            p.check_host("example.com"),
            NetworkDecision::Denied(SandboxError::NetworkDisabled)
        ));
    }

    #[test]
    fn empty_list_allows_everything() {
        let p = NetworkPolicy::default();
        assert_eq!(p.check_host("any.example.com"), NetworkDecision::Allowed);
    }

    #[test]
    fn bare_domain_matches_self_and_subs() {
        let p = NetworkPolicy {
            disabled: false,
            allowed_domains: vec!["example.com".into()],
        };
        assert_eq!(p.check_host("example.com"), NetworkDecision::Allowed);
        assert_eq!(p.check_host("api.example.com"), NetworkDecision::Allowed);
        assert!(matches!(
            p.check_host("evil.net"),
            NetworkDecision::Denied(_)
        ));
    }

    #[test]
    fn wildcard_matches_subdomains() {
        let p = NetworkPolicy {
            disabled: false,
            allowed_domains: vec!["*.example.com".into()],
        };
        assert_eq!(p.check_host("x.example.com"), NetworkDecision::Allowed);
        assert_eq!(p.check_host("example.com"), NetworkDecision::Allowed);
        assert!(matches!(
            p.check_host("other.net"),
            NetworkDecision::Denied(_)
        ));
    }

    #[test]
    fn host_matching_is_case_insensitive() {
        let p = NetworkPolicy {
            disabled: false,
            allowed_domains: vec!["EXAMPLE.com".into()],
        };
        assert_eq!(p.check_host("Example.Com"), NetworkDecision::Allowed);
    }

    #[test]
    fn url_check_uses_host() {
        let p = NetworkPolicy {
            disabled: false,
            allowed_domains: vec!["example.com".into()],
        };
        assert_eq!(
            p.check_url("https://api.example.com/x"),
            NetworkDecision::Allowed
        );
        assert!(matches!(
            p.check_url("https://evil.net/"),
            NetworkDecision::Denied(_)
        ));
    }

    #[test]
    fn url_check_rejects_invalid_url() {
        let p = NetworkPolicy::default();
        assert!(matches!(
            p.check_url("not a url"),
            NetworkDecision::Denied(_)
        ));
    }
}

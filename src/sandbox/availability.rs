//! Detect whether the OS-level sandbox primitive is usable on this host.
//!
//! Results:
//! - Linux (and WSL2): looks for `bwrap` on `$PATH`
//! - macOS: looks for `/usr/bin/sandbox-exec`
//! - Windows: always `Unavailable` for now (Restricted Token support TBD)
//!
//! Called once per sandbox construction and cached in
//! [`crate::sandbox::SandboxPolicy`].

#![allow(dead_code)]
// The `Available` variant, `is_available`, and `Mechanism::WindowsRestrictedToken`
// are only constructed on some targets — Windows builds see them as dead even
// though Linux/macOS builds use them. Allowing dead_code here keeps the enum
// exhaustive for pattern matching on every platform.

use std::sync::OnceLock;

/// Outcome of the availability probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    /// OS primitive is ready to use. Carries the concrete mechanism for UI.
    Available(Mechanism),
    /// OS primitive is not usable. Contains a human-readable reason.
    Unavailable {
        platform: &'static str,
        reason: String,
    },
}

/// Which OS-level primitive is wired up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mechanism {
    /// Linux / WSL2 bubblewrap (`bwrap`).
    Bubblewrap,
    /// macOS Seatbelt (`sandbox-exec`).
    Seatbelt,
    /// Windows Restricted Token + Job Object (future work).
    WindowsRestrictedToken,
}

impl Mechanism {
    pub fn as_str(self) -> &'static str {
        match self {
            Mechanism::Bubblewrap => "bubblewrap",
            Mechanism::Seatbelt => "sandbox-exec (Seatbelt)",
            Mechanism::WindowsRestrictedToken => "Windows Restricted Token + Job Object",
        }
    }
}

impl Availability {
    pub fn is_available(&self) -> bool {
        matches!(self, Availability::Available(_))
    }

    pub fn mechanism(&self) -> Option<Mechanism> {
        match self {
            Availability::Available(m) => Some(*m),
            _ => None,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Availability::Available(m) => {
                format!("OS-level sandbox available via {}", m.as_str())
            }
            Availability::Unavailable { platform, reason } => {
                format!("OS-level sandbox unavailable on {}: {}", platform, reason)
            }
        }
    }
}

static CACHED: OnceLock<Availability> = OnceLock::new();

/// Detect availability, caching the result for the lifetime of the process.
///
/// The check itself is cheap (a `which`-style lookup), but we still cache
/// to guarantee a stable answer across repeated sandbox constructions.
pub fn detect_availability() -> Availability {
    CACHED.get_or_init(probe).clone()
}

fn probe() -> Availability {
    #[cfg(target_os = "linux")]
    {
        match which::which("bwrap") {
            Ok(_) => Availability::Available(Mechanism::Bubblewrap),
            Err(e) => Availability::Unavailable {
                platform: "linux",
                reason: format!(
                    "'bwrap' not found on PATH ({}). Install bubblewrap (e.g. \
                     `apt-get install bubblewrap socat`) to enable OS-level \
                     sandboxing.",
                    e
                ),
            },
        }
    }

    #[cfg(target_os = "macos")]
    {
        let p = std::path::Path::new("/usr/bin/sandbox-exec");
        if p.exists() {
            Availability::Available(Mechanism::Seatbelt)
        } else {
            Availability::Unavailable {
                platform: "macos",
                reason: "/usr/bin/sandbox-exec not found (Seatbelt is a system \
                         binary and should be present on all macOS versions; \
                         report this as a bug)"
                    .into(),
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        Availability::Unavailable {
            platform: "windows",
            reason: "Windows Restricted Token + Job Object support is not \
                     wired up yet. Rust-level policy checks still apply; OS-level \
                     enforcement is deferred to a follow-up (issue #8)."
                .into(),
        }
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        Availability::Unavailable {
            platform: "unknown",
            reason: "no sandbox back-end is implemented for this platform".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_stable_answer() {
        let a = detect_availability();
        let b = detect_availability();
        assert_eq!(a, b);
    }

    #[test]
    fn describe_is_non_empty() {
        assert!(!detect_availability().describe().is_empty());
    }
}

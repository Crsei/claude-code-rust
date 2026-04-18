#![allow(dead_code)]
//! Central registry of cc-rust runtime persistence paths.
//!
//! All runtime data — sessions, logs, credentials, transcripts — is resolved
//! through this module. The resolution chain is:
//!
//! 1. `CC_RUST_HOME` env var (trim non-empty) → use as-is.
//! 2. `dirs::home_dir().join(".cc-rust")`.
//! 3. `std::env::temp_dir().join("cc-rust")` (non-persistent, warns once).
//!
//! Functions return `PathBuf` unconditionally; they never fail. Creation of
//! the directory is the caller's responsibility.

use std::path::PathBuf;
use std::sync::Once;

static TEMP_FALLBACK_WARN: Once = Once::new();

/// Return the cc-rust data root directory.
///
/// See module-level docs for the resolution chain.
pub fn data_root() -> PathBuf {
    if let Ok(override_dir) = std::env::var("CC_RUST_HOME") {
        if !override_dir.trim().is_empty() {
            return PathBuf::from(override_dir);
        }
    }

    if let Some(home) = dirs::home_dir() {
        return home.join(".cc-rust");
    }

    let tmp = std::env::temp_dir().join("cc-rust");
    TEMP_FALLBACK_WARN.call_once(|| {
        tracing::warn!(
            path = %tmp.display(),
            "unable to resolve home directory; cc-rust data will be written to a \
             non-persistent temp location. Set CC_RUST_HOME to override."
        );
    });
    tmp
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    #[serial]
    fn data_root_uses_env_override() {
        let _g = EnvGuard::set("CC_RUST_HOME", "/tmp/cc-rust-test-override");
        assert_eq!(data_root(), PathBuf::from("/tmp/cc-rust-test-override"));
    }

    #[test]
    #[serial]
    fn data_root_ignores_empty_env() {
        let _g = EnvGuard::set("CC_RUST_HOME", "");
        let root = data_root();
        assert!(
            root.ends_with(".cc-rust") || root.file_name().map(|n| n == "cc-rust").unwrap_or(false),
            "expected home fallback or temp fallback, got {}",
            root.display()
        );
    }

    #[test]
    #[serial]
    fn data_root_ignores_whitespace_env() {
        let _g = EnvGuard::set("CC_RUST_HOME", "   ");
        let root = data_root();
        assert!(
            root.ends_with(".cc-rust") || root.file_name().map(|n| n == "cc-rust").unwrap_or(false),
            "expected home fallback or temp fallback, got {}",
            root.display()
        );
    }

    #[test]
    #[serial]
    fn data_root_respects_env_with_whitespace_padding() {
        let _g = EnvGuard::set("CC_RUST_HOME", " /tmp/padded ");
        assert_eq!(data_root(), PathBuf::from(" /tmp/padded "));
    }

    #[test]
    #[serial]
    fn data_root_temp_fallback_best_effort() {
        let _g1 = EnvGuard::unset("CC_RUST_HOME");
        if dirs::home_dir().is_some() {
            eprintln!("skipping: dirs::home_dir() still resolvable via OS APIs");
            return;
        }
        let root = data_root();
        assert!(
            root.starts_with(std::env::temp_dir()),
            "expected temp fallback, got {}",
            root.display()
        );
    }
}

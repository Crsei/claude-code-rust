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

use chrono::{DateTime, Local};
use std::path::Path;
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

// ----- Global paths (under data_root) --------------------------------------

pub fn sessions_dir() -> PathBuf {
    data_root().join("sessions")
}

pub fn logs_dir() -> PathBuf {
    data_root().join("logs")
}

/// `{data_root}/logs/YYYY/MM/YYYY-MM-DD.md` — daemon daily log layout.
pub fn daily_log_path(now: DateTime<Local>) -> PathBuf {
    logs_dir()
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string())
        .join(now.format("%Y-%m-%d.md").to_string())
}

pub fn credentials_path() -> PathBuf {
    data_root().join("credentials.json")
}

pub fn runs_dir(session_id: &str) -> PathBuf {
    data_root().join("runs").join(session_id)
}

pub fn exports_dir() -> PathBuf {
    data_root().join("exports")
}

pub fn audits_dir() -> PathBuf {
    data_root().join("audits")
}

pub fn transcripts_dir() -> PathBuf {
    data_root().join("transcripts")
}

pub fn memory_dir_global() -> PathBuf {
    data_root().join("memory")
}

pub fn session_insights_dir() -> PathBuf {
    data_root().join("session-insights")
}

pub fn plugins_dir() -> PathBuf {
    data_root().join("plugins")
}

pub fn skills_dir_global() -> PathBuf {
    data_root().join("skills")
}

pub fn teams_dir() -> PathBuf {
    data_root().join("teams")
}

pub fn tasks_dir() -> PathBuf {
    data_root().join("tasks")
}

/// `{data_root}/projects/{sanitized_cwd}/memory/team/`
pub fn team_memory_dir(cwd: &Path) -> PathBuf {
    let sanitized: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    data_root()
        .join("projects")
        .join(sanitized)
        .join("memory")
        .join("team")
}

// ----- Project-local paths (under cwd) -------------------------------------

/// `{cwd}/.cc-rust/` — project-level settings / memory / skills root.
///
/// Note: call sites currently inline `cwd.join(".cc-rust")`; this helper exists
/// for future consolidation. Remove the `#[allow(dead_code)]` once migrated.
#[allow(dead_code)]
pub fn project_cc_rust_dir(cwd: &Path) -> PathBuf {
    cwd.join(".cc-rust")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
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

    #[test]
    #[serial]
    fn partition_functions_all_root_under_data_root() {
        let _g = EnvGuard::set("CC_RUST_HOME", "/tmp/cc-rust-partition-test");
        let base = PathBuf::from("/tmp/cc-rust-partition-test");
        assert_eq!(sessions_dir(), base.join("sessions"));
        assert_eq!(logs_dir(), base.join("logs"));
        assert_eq!(credentials_path(), base.join("credentials.json"));
        assert_eq!(runs_dir("abc"), base.join("runs").join("abc"));
        assert_eq!(exports_dir(), base.join("exports"));
        assert_eq!(audits_dir(), base.join("audits"));
        assert_eq!(transcripts_dir(), base.join("transcripts"));
        assert_eq!(memory_dir_global(), base.join("memory"));
        assert_eq!(session_insights_dir(), base.join("session-insights"));
        assert_eq!(plugins_dir(), base.join("plugins"));
        assert_eq!(skills_dir_global(), base.join("skills"));
        assert_eq!(teams_dir(), base.join("teams"));
        assert_eq!(tasks_dir(), base.join("tasks"));
    }

    #[test]
    #[serial]
    fn daily_log_path_builds_yyyy_mm_dd_layout() {
        let _g = EnvGuard::set("CC_RUST_HOME", "/tmp/cc-rust-dlp");
        let dt = Local.with_ymd_and_hms(2026, 4, 18, 10, 30, 0).unwrap();
        let p = daily_log_path(dt);
        let s = p.to_string_lossy().replace('\\', "/");
        assert!(s.ends_with("logs/2026/04/2026-04-18.md"), "got {}", s);
    }

    #[test]
    #[serial]
    fn team_memory_dir_sanitizes_cwd() {
        let _g = EnvGuard::set("CC_RUST_HOME", "/tmp/cc-rust-tmd");
        let cwd = Path::new("/home/user/My Project/sub");
        let p = team_memory_dir(cwd);
        let s = p.to_string_lossy().replace('\\', "/");
        assert!(
            s.ends_with("_home_user_My_Project_sub/memory/team")
                || s.ends_with("/_home_user_My_Project_sub/memory/team"),
            "unexpected path: {}",
            s
        );
    }

    #[test]
    #[serial]
    fn project_cc_rust_dir_is_cwd_relative() {
        // Not affected by CC_RUST_HOME.
        let _g = EnvGuard::set("CC_RUST_HOME", "/tmp/ignored");
        assert_eq!(
            project_cc_rust_dir(Path::new("/foo/bar")),
            PathBuf::from("/foo/bar/.cc-rust")
        );
    }
}

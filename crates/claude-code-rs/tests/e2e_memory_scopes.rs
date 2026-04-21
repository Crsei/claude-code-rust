//! E2E tests for the expanded memory scope system (issue #45).
//!
//! Exercises `cc_session::memdir` + `cc_config::paths` + the `/memory`
//! command surface together:
//! - All four scopes (Global, Project, Team, Auto) resolve to
//!   well-defined paths under a sandboxed `CC_RUST_HOME`.
//! - Writes/reads round-trip correctly across every scope.
//! - The layered settings loader persists `autoMemoryEnabled`.
//!
//! These tests are hermetic: each sets `CC_RUST_HOME` to a tempdir so
//! they never touch `~/.cc-rust/`.
//!
//! Run with: `cargo test --test e2e_memory_scopes`

use cc_config::paths;
use cc_config::settings::{self, RawSettings};
use cc_session::memdir::{
    delete_memory, list_memories, memory_dir, read_memory, write_memory, MemoryScope,
};
use serial_test::serial;
use tempfile::TempDir;

struct CcRustHomeGuard {
    previous: Option<String>,
}

impl CcRustHomeGuard {
    fn set(path: &std::path::Path) -> Self {
        let previous = std::env::var("CC_RUST_HOME").ok();
        std::env::set_var("CC_RUST_HOME", path);
        Self { previous }
    }
}

impl Drop for CcRustHomeGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => std::env::set_var("CC_RUST_HOME", v),
            None => std::env::remove_var("CC_RUST_HOME"),
        }
    }
}

#[test]
#[serial]
fn memory_dir_resolves_every_scope_under_data_root() {
    let root = TempDir::new().expect("tmp root");
    let _g = CcRustHomeGuard::set(root.path());

    let cwd = root.path().join("my_project");
    std::fs::create_dir_all(&cwd).unwrap();

    assert_eq!(
        memory_dir(MemoryScope::Global, &cwd).unwrap(),
        root.path().join("memory")
    );
    assert_eq!(
        memory_dir(MemoryScope::Project, &cwd).unwrap(),
        cwd.join(".cc-rust").join("memory")
    );

    // The team dir is rooted under data_root/projects/<sanitized>/memory/team
    let team = memory_dir(MemoryScope::Team, &cwd).unwrap();
    let s = team.to_string_lossy().replace('\\', "/");
    assert!(
        s.starts_with(&root.path().to_string_lossy().replace('\\', "/")),
        "team path {} should be under data_root {}",
        team.display(),
        root.path().display()
    );
    assert!(
        s.ends_with("/memory/team"),
        "team path {} should end with /memory/team",
        team.display()
    );

    // Auto scope lives at data_root/auto_memory — matches the helper too.
    assert_eq!(
        memory_dir(MemoryScope::Auto, &cwd).unwrap(),
        paths::auto_memory_dir()
    );
    assert_eq!(
        memory_dir(MemoryScope::Auto, &cwd).unwrap(),
        root.path().join("auto_memory")
    );
}

#[test]
#[serial]
fn every_scope_supports_write_list_delete() {
    let root = TempDir::new().expect("tmp root");
    let _g = CcRustHomeGuard::set(root.path());

    let cwd = root.path().join("ws");
    std::fs::create_dir_all(&cwd).unwrap();

    for scope in [
        MemoryScope::Global,
        MemoryScope::Project,
        MemoryScope::Team,
        MemoryScope::Auto,
    ] {
        let key = format!("e2e-{}", scope.as_str());
        let value = format!("value for {}", scope.as_str());

        let written = write_memory(&key, &value, "e2e", scope, &cwd).unwrap();
        assert_eq!(written.key, key);
        assert_eq!(written.value, value);

        let read = read_memory(&key, scope, &cwd).unwrap();
        assert_eq!(read.key, key);

        let all = list_memories(scope, &cwd).unwrap();
        assert!(all.iter().any(|e| e.key == key));

        assert!(delete_memory(&key, scope, &cwd).unwrap());
        assert!(!delete_memory(&key, scope, &cwd).unwrap()); // idempotent
    }
}

#[test]
#[serial]
fn auto_memory_enabled_roundtrips_through_settings_json() {
    let root = TempDir::new().expect("tmp root");
    let _g = CcRustHomeGuard::set(root.path());

    let path = settings::user_settings_path();
    let mut raw = RawSettings::default();
    raw.auto_memory_enabled = Some(true);
    settings::write_settings_file(&path, &raw).expect("write settings");

    let loaded = settings::load_effective(&std::path::PathBuf::from(root.path()))
        .expect("load effective");
    assert_eq!(
        loaded.effective.auto_memory_enabled,
        Some(true),
        "autoMemoryEnabled should survive a round-trip through settings.json"
    );
}

#[test]
#[serial]
fn auto_memory_dir_helper_matches_data_root_layout() {
    let root = TempDir::new().expect("tmp root");
    let _g = CcRustHomeGuard::set(root.path());

    assert_eq!(paths::auto_memory_dir(), root.path().join("auto_memory"));
    // Distinct from the curated global memory dir so purge-all semantics
    // don't clobber hand-written entries.
    assert_ne!(paths::auto_memory_dir(), paths::memory_dir_global());
}

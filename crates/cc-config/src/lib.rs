//! Configuration management — extracted as a workspace crate in Phase 3
//! (issue #72).
//!
//! Owns:
//! - `settings.json` loader + effective-settings merge layer
//! - `CLAUDE.md` discovery + injection
//! - Data-root path helpers (`~/.cc-rust/` or `$CC_RUST_HOME`)
//! - Feature-gate system (`FEATURE_*` env vars)
//! - Config validation warnings
//! - `runtime_settings::SettingsJson` — the runtime projection of effective
//!   settings previously in `types::app_state::SettingsJson` (moved here to
//!   let `config::validation` read it without a reverse dep back into the
//!   root crate).

pub mod claude_md;
pub mod constants;
pub mod features;
pub mod paths;
pub mod runtime_settings;
pub mod settings;
pub mod validation;

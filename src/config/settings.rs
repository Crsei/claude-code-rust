//! Settings management (layered, source-aware).
//!
//! # Layered configuration
//!
//! Configuration is merged from the following sources, in **ascending**
//! priority (higher overrides lower):
//!
//! 1. `Managed` — policy-level settings. Windows prefers
//!    `%ProgramData%\cc-rust\settings.json`; other platforms use
//!    `/etc/cc-rust/managed-settings.json`. Overridable with
//!    `CC_RUST_MANAGED_SETTINGS`.
//! 2. `User` — `{data_root}/settings.json` (i.e. `~/.cc-rust/settings.json`
//!    or `$CC_RUST_HOME/settings.json`).
//! 3. `Project` — `.cc-rust/settings.json` in CWD or any ancestor directory.
//! 4. `Local` — `.cc-rust/settings.local.json` next to the project settings
//!    (intended for gitignored per-machine overrides).
//! 5. `Env` — a handful of CLAUDE_* / CC_* environment variables.
//! 6. `Cli` — command-line flags (applied by the caller, not this module).
//!
//! The merge produces an [`EffectiveSettings`] together with a
//! [`SourceMap`] that records which layer won for each key.
//!
//! # Backward compatibility
//!
//! The legacy names [`GlobalConfig`], [`ProjectConfig`], [`MergedConfig`],
//! [`load_global_config`], [`load_project_config`], [`merge_configs`], and
//! [`load_and_merge`] are preserved so existing callers keep compiling.
//! New callers should prefer [`load_effective`], [`RawSettings`], and
//! [`EffectiveSettings`].

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Source tracking
// ---------------------------------------------------------------------------

/// Origin of a single configuration value.
///
/// Used by [`SourceMap`] so the user can introspect where each effective
/// value came from (`/config show --effective`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingsSource {
    /// Compiled-in default (no file / env provided a value).
    Default,
    /// Managed / policy-level settings.
    Managed,
    /// User-level settings (`~/.cc-rust/settings.json`).
    User,
    /// Project-level settings (`.cc-rust/settings.json`).
    Project,
    /// Project-local overrides (`.cc-rust/settings.local.json`).
    Local,
    /// Environment variable override.
    Env,
    /// CLI flag override (set by `main.rs` after loading).
    Cli,
}

impl SettingsSource {
    /// Priority ranking — higher wins in a merge.
    ///
    /// Exposed so callers (e.g. `/config sources`) can break ties or sort
    /// by priority order without re-implementing the table.
    #[allow(dead_code)]
    pub fn rank(self) -> u8 {
        match self {
            SettingsSource::Default => 0,
            SettingsSource::Managed => 1,
            SettingsSource::User => 2,
            SettingsSource::Project => 3,
            SettingsSource::Local => 4,
            SettingsSource::Env => 5,
            SettingsSource::Cli => 6,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SettingsSource::Default => "default",
            SettingsSource::Managed => "managed",
            SettingsSource::User => "user",
            SettingsSource::Project => "project",
            SettingsSource::Local => "local",
            SettingsSource::Env => "env",
            SettingsSource::Cli => "cli",
        }
    }
}

/// Per-key provenance for merged settings. Uses a `BTreeMap` so the output
/// of `/config show` is deterministic.
pub type SourceMap = BTreeMap<String, SettingsSource>;

// ---------------------------------------------------------------------------
// Typed sub-structures for richer settings
// ---------------------------------------------------------------------------

/// Permissions section of settings.json.
///
/// Mirrors the Claude Code TS `PermissionsSettings` shape at a high level.
/// Missing fields are `None`; empty arrays are treated the same as missing
/// for merge purposes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionsSettings {
    /// Permission mode. One of: `default`, `ask`, `auto`, `bypass`, `plan`.
    pub default_mode: Option<String>,
    /// Tools that are always allowed (patterns).
    pub allow: Vec<String>,
    /// Tools that are always asked before execution.
    pub ask: Vec<String>,
    /// Tools that are always denied.
    pub deny: Vec<String>,
    /// Additional working directories the tools may access.
    pub additional_directories: Vec<String>,
    /// Whether `bypass` mode should be allowed at runtime.
    pub enable_bypass_mode: Option<bool>,
    /// Whether `auto` mode should be allowed at runtime.
    pub enable_auto_mode: Option<bool>,
    /// Unknown fields so forward-compat is preserved.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl PermissionsSettings {
    pub fn is_effectively_empty(&self) -> bool {
        self.default_mode.is_none()
            && self.allow.is_empty()
            && self.ask.is_empty()
            && self.deny.is_empty()
            && self.additional_directories.is_empty()
            && self.enable_bypass_mode.is_none()
            && self.enable_auto_mode.is_none()
            && self.extra.is_empty()
    }
}

/// Sandbox section of settings.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SandboxSettings {
    /// Enable sandbox for shell / tool execution.
    pub enabled: Option<bool>,
    /// Sandbox profile / mode identifier.
    pub mode: Option<String>,
    /// Per-command allow list.
    pub allowed_commands: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Status-line configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct StatusLineSettings {
    /// Status-line type, e.g. `none`, `minimal`, `command`, `script`.
    pub r#type: Option<String>,
    /// Inline command to execute for `command` type.
    pub command: Option<String>,
    /// Path to a script for `script` type.
    pub script: Option<String>,
    /// Optional format template.
    pub format: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Spinner-tip configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SpinnerTipsSettings {
    pub enabled: Option<bool>,
    pub interval_ms: Option<u64>,
    pub custom_tips: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

// ---------------------------------------------------------------------------
// RawSettings — on-disk shape of a single settings file
// ---------------------------------------------------------------------------

/// On-disk shape of a single `settings.json` (or `settings.local.json`,
/// managed settings, etc.). All fields are optional.
///
/// Unknown keys fall into [`RawSettings::extra`] to preserve forward
/// compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct RawSettings {
    // -- Core identity --------------------------------------------------
    pub model: Option<String>,
    pub backend: Option<String>,
    pub theme: Option<String>,
    pub verbose: Option<bool>,

    // -- Permissions / sandbox -----------------------------------------
    /// Legacy top-level permission mode (e.g. "auto"). Prefer
    /// `permissions.defaultMode`. If both are present, nested wins.
    pub permission_mode: Option<String>,
    /// Legacy flat allowed-tools list. Prefer `permissions.allow`.
    pub allowed_tools: Option<Vec<String>>,
    pub permissions: Option<PermissionsSettings>,
    pub sandbox: Option<SandboxSettings>,

    // -- Hooks ----------------------------------------------------------
    /// Event → config value mapping (deserialized by tools/hooks).
    pub hooks: Option<HashMap<String, Value>>,

    // -- UI / UX --------------------------------------------------------
    pub status_line: Option<StatusLineSettings>,
    pub output_style: Option<String>,
    pub language: Option<String>,
    pub voice_enabled: Option<bool>,
    pub editor_mode: Option<String>,
    pub view_mode: Option<String>,
    pub spinner_tips: Option<SpinnerTipsSettings>,
    pub terminal_progress_bar_enabled: Option<bool>,

    // -- Model / effort -------------------------------------------------
    pub available_models: Option<Vec<String>>,
    pub effort_level: Option<String>,
    pub fast_mode: Option<bool>,
    pub fast_mode_per_session_opt_in: Option<bool>,

    // -- Modes / integrations ------------------------------------------
    pub teammate_mode: Option<bool>,
    #[serde(rename = "claudeInChromeDefaultEnabled")]
    pub claude_in_chrome_default_enabled: Option<bool>,

    // -- Prompts --------------------------------------------------------
    pub system_prompt: Option<String>,

    // -- Credentials ----------------------------------------------------
    /// API key override. User-level only; strongly discouraged. Redacted in
    /// source-map output.
    pub api_key: Option<String>,

    // -- Arbitrary passthrough ------------------------------------------
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl RawSettings {
    /// Merge `other` **on top of** `self`. Mutates `self` in place and
    /// records, in `sources`, every key that `other` provided.
    fn merge_from(&mut self, other: RawSettings, source: SettingsSource, sources: &mut SourceMap) {
        macro_rules! merge_opt {
            ($field:ident, $key:expr) => {
                if let Some(v) = other.$field {
                    self.$field = Some(v);
                    sources.insert($key.to_string(), source);
                }
            };
        }

        merge_opt!(model, "model");
        merge_opt!(backend, "backend");
        merge_opt!(theme, "theme");
        merge_opt!(verbose, "verbose");
        merge_opt!(permission_mode, "permissionMode");

        if let Some(list) = other.allowed_tools {
            let merged = merge_str_lists(self.allowed_tools.as_deref(), Some(&list));
            self.allowed_tools = Some(merged);
            sources.insert("allowedTools".to_string(), source);
        }

        if let Some(perms) = other.permissions {
            if !perms.is_effectively_empty() {
                self.permissions = Some(merge_permissions(self.permissions.take(), perms));
                sources.insert("permissions".to_string(), source);
            }
        }

        merge_opt!(sandbox, "sandbox");

        if let Some(hooks) = other.hooks {
            let mut merged = self.hooks.take().unwrap_or_default();
            for (k, v) in hooks {
                merged.insert(k, v);
            }
            self.hooks = Some(merged);
            sources.insert("hooks".to_string(), source);
        }

        merge_opt!(status_line, "statusLine");
        merge_opt!(output_style, "outputStyle");
        merge_opt!(language, "language");
        merge_opt!(voice_enabled, "voiceEnabled");
        merge_opt!(editor_mode, "editorMode");
        merge_opt!(view_mode, "viewMode");
        merge_opt!(spinner_tips, "spinnerTips");
        merge_opt!(
            terminal_progress_bar_enabled,
            "terminalProgressBarEnabled"
        );
        merge_opt!(available_models, "availableModels");
        merge_opt!(effort_level, "effortLevel");
        merge_opt!(fast_mode, "fastMode");
        merge_opt!(
            fast_mode_per_session_opt_in,
            "fastModePerSessionOptIn"
        );
        merge_opt!(teammate_mode, "teammateMode");
        merge_opt!(
            claude_in_chrome_default_enabled,
            "claudeInChromeDefaultEnabled"
        );
        merge_opt!(system_prompt, "systemPrompt");
        merge_opt!(api_key, "apiKey");

        for (k, v) in other.extra {
            self.extra.insert(k.clone(), v);
            sources.insert(k, source);
        }
    }
}

fn merge_permissions(base: Option<PermissionsSettings>, over: PermissionsSettings) -> PermissionsSettings {
    let mut out = base.unwrap_or_default();
    if over.default_mode.is_some() {
        out.default_mode = over.default_mode;
    }
    out.allow = merge_str_lists(Some(&out.allow), Some(&over.allow));
    out.ask = merge_str_lists(Some(&out.ask), Some(&over.ask));
    out.deny = merge_str_lists(Some(&out.deny), Some(&over.deny));
    out.additional_directories = merge_str_lists(
        Some(&out.additional_directories),
        Some(&over.additional_directories),
    );
    if over.enable_bypass_mode.is_some() {
        out.enable_bypass_mode = over.enable_bypass_mode;
    }
    if over.enable_auto_mode.is_some() {
        out.enable_auto_mode = over.enable_auto_mode;
    }
    for (k, v) in over.extra {
        out.extra.insert(k, v);
    }
    out
}

fn merge_str_lists(base: Option<&[String]>, over: Option<&[String]>) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(b) = base {
        out.extend_from_slice(b);
    }
    if let Some(o) = over {
        for item in o {
            if !out.contains(item) {
                out.push(item.clone());
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Backward-compat type aliases
// ---------------------------------------------------------------------------

/// Legacy alias — global/user settings file shape.
///
/// Prefer [`RawSettings`] in new code. Kept so that historic call sites
/// (`use settings::GlobalConfig;`) continue to compile after the refactor.
#[allow(dead_code)]
pub type GlobalConfig = RawSettings;

/// Legacy alias — project settings file shape.
///
/// Prefer [`RawSettings`] in new code. Kept for the same reason as
/// [`GlobalConfig`].
#[allow(dead_code)]
pub type ProjectConfig = RawSettings;

/// Merged runtime configuration. See [`EffectiveSettings`] for the new,
/// source-aware form.
pub type MergedConfig = EffectiveSettings;

// ---------------------------------------------------------------------------
// EffectiveSettings — runtime-ready, merged form
// ---------------------------------------------------------------------------

/// Fully-merged, runtime-ready settings.
///
/// Fields that have reasonable defaults are fully materialised (e.g.
/// `verbose: bool` rather than `Option<bool>`). Fields that have no
/// meaningful default stay `Option`.
///
/// Paired with a [`SourceMap`] via [`LoadedSettings`].
#[derive(Debug, Clone, Default)]
pub struct EffectiveSettings {
    // -- Legacy (consumed by main.rs) ----------------------------------
    pub model: Option<String>,
    pub backend: Option<String>,
    pub theme: Option<String>,
    pub verbose: bool,
    pub permission_mode: Option<String>,
    #[allow(dead_code)]
    pub allowed_tools: Vec<String>,
    #[allow(dead_code)]
    pub system_prompt: Option<String>,
    pub hooks: HashMap<String, Value>,
    pub claude_in_chrome_default_enabled: Option<bool>,
    pub api_key: Option<String>,
    #[allow(dead_code)]
    pub extra: HashMap<String, Value>,

    // -- New typed fields ----------------------------------------------
    pub permissions: PermissionsSettings,
    pub sandbox: SandboxSettings,
    pub status_line: StatusLineSettings,
    pub spinner_tips: SpinnerTipsSettings,
    pub output_style: Option<String>,
    pub language: Option<String>,
    pub voice_enabled: Option<bool>,
    pub editor_mode: Option<String>,
    pub view_mode: Option<String>,
    pub terminal_progress_bar_enabled: Option<bool>,
    pub available_models: Vec<String>,
    pub effort_level: Option<String>,
    pub fast_mode: Option<bool>,
    pub fast_mode_per_session_opt_in: Option<bool>,
    pub teammate_mode: Option<bool>,
}

impl EffectiveSettings {
    fn from_raw(raw: RawSettings) -> Self {
        let mut perms = raw.permissions.unwrap_or_default();
        // Fold legacy top-level fields into the nested struct so downstream
        // code only needs to look in one place.
        if perms.default_mode.is_none() {
            perms.default_mode = raw.permission_mode.clone();
        }
        if let Some(legacy) = raw.allowed_tools.as_ref() {
            perms.allow = merge_str_lists(Some(&perms.allow), Some(legacy));
        }

        Self {
            model: raw.model,
            backend: raw.backend,
            theme: raw.theme,
            verbose: raw.verbose.unwrap_or(false),
            permission_mode: perms.default_mode.clone().or(raw.permission_mode),
            allowed_tools: perms.allow.clone(),
            system_prompt: raw.system_prompt,
            hooks: raw.hooks.unwrap_or_default(),
            claude_in_chrome_default_enabled: raw.claude_in_chrome_default_enabled,
            api_key: raw.api_key,
            extra: raw.extra,
            permissions: perms,
            sandbox: raw.sandbox.unwrap_or_default(),
            status_line: raw.status_line.unwrap_or_default(),
            spinner_tips: raw.spinner_tips.unwrap_or_default(),
            output_style: raw.output_style,
            language: raw.language,
            voice_enabled: raw.voice_enabled,
            editor_mode: raw.editor_mode,
            view_mode: raw.view_mode,
            terminal_progress_bar_enabled: raw.terminal_progress_bar_enabled,
            available_models: raw.available_models.unwrap_or_default(),
            effort_level: raw.effort_level,
            fast_mode: raw.fast_mode,
            fast_mode_per_session_opt_in: raw.fast_mode_per_session_opt_in,
            teammate_mode: raw.teammate_mode,
        }
    }
}

// ---------------------------------------------------------------------------
// LoadedSettings — effective + raw layers + source map
// ---------------------------------------------------------------------------

/// Result of [`load_effective`]. Holds the merged [`EffectiveSettings`]
/// and a [`SourceMap`] recording which layer provided each key, plus the
/// raw per-layer contents for diagnostics.
#[derive(Debug, Clone, Default)]
pub struct LoadedSettings {
    pub effective: EffectiveSettings,
    pub sources: SourceMap,
    pub managed: Option<RawSettings>,
    pub user: Option<RawSettings>,
    pub project: Option<RawSettings>,
    pub local: Option<RawSettings>,
    /// Paths that were actually read (present on disk).
    pub loaded_paths: Vec<(SettingsSource, PathBuf)>,
}

impl LoadedSettings {
    /// Source of a specific key (e.g. `"model"`, `"permissions"`).
    ///
    /// Returns [`SettingsSource::Default`] if no layer provided the key.
    /// Used by `/config sources` and tests; reserved for downstream callers
    /// that want to inspect provenance without iterating the full map.
    #[allow(dead_code)]
    pub fn source_of(&self, key: &str) -> SettingsSource {
        self.sources
            .get(key)
            .copied()
            .unwrap_or(SettingsSource::Default)
    }
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

/// Global cc-rust data directory. Never fails — falls back to a temp dir.
pub fn global_claude_dir() -> Result<PathBuf> {
    Ok(crate::config::paths::data_root())
}

/// Path to the user-level settings file.
pub fn user_settings_path() -> PathBuf {
    crate::config::paths::data_root().join("settings.json")
}

/// Path to the managed/policy settings file, if one is configured.
///
/// Resolution:
///   1. `CC_RUST_MANAGED_SETTINGS` env (if non-empty).
///   2. Windows: `%ProgramData%\cc-rust\settings.json` (or
///      `C:\ProgramData\cc-rust\settings.json` if the env var is absent).
///   3. Other: `/etc/cc-rust/managed-settings.json`.
pub fn managed_settings_path() -> PathBuf {
    if let Ok(p) = std::env::var("CC_RUST_MANAGED_SETTINGS") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    #[cfg(windows)]
    {
        let base = std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
        base.join("cc-rust").join("settings.json")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/cc-rust/managed-settings.json")
    }
}

/// Return the path to the nearest ancestor project settings directory
/// (`.cc-rust/`) or `None`.
fn find_project_dir(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd.to_path_buf();
    loop {
        let candidate = dir.join(".cc-rust");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Search `cwd` and its ancestors for `.cc-rust/settings.json`.
fn find_project_config(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd.to_path_buf();
    loop {
        let candidate = dir.join(".cc-rust").join("settings.json");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Search for `.cc-rust/settings.local.json`.
fn find_local_config(cwd: &Path) -> Option<PathBuf> {
    let mut dir = cwd.to_path_buf();
    loop {
        let candidate = dir.join(".cc-rust").join("settings.local.json");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ---------------------------------------------------------------------------
// Loaders
// ---------------------------------------------------------------------------

fn load_raw_from(path: &Path) -> Result<Option<RawSettings>> {
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let raw: RawSettings = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(raw))
}

/// Load the user-level settings. Returns `Ok(RawSettings::default())` if
/// the file does not exist.
///
/// Convenience wrapper kept for callers that only want one layer; the full
/// stack is loaded via [`load_effective`].
#[allow(dead_code)]
pub fn load_global_config() -> Result<RawSettings> {
    Ok(load_raw_from(&user_settings_path())?.unwrap_or_default())
}

/// Load the project-level settings. Returns defaults if none is found.
#[allow(dead_code)]
pub fn load_project_config(cwd: &Path) -> Result<RawSettings> {
    match find_project_config(cwd) {
        Some(p) => Ok(load_raw_from(&p)?.unwrap_or_default()),
        None => Ok(RawSettings::default()),
    }
}

/// Load project-local overrides (`.cc-rust/settings.local.json`).
#[allow(dead_code)]
pub fn load_local_config(cwd: &Path) -> Result<RawSettings> {
    match find_local_config(cwd) {
        Some(p) => Ok(load_raw_from(&p)?.unwrap_or_default()),
        None => Ok(RawSettings::default()),
    }
}

/// Load managed / policy settings, if a managed settings file exists on
/// disk. Errors reading an existing file are surfaced; a missing file is
/// treated as "no managed layer".
#[allow(dead_code)]
pub fn load_managed_config() -> Result<RawSettings> {
    Ok(load_raw_from(&managed_settings_path())?.unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Merge / env overrides
// ---------------------------------------------------------------------------

/// Merge exactly two layers (global then project). Preserved for
/// backward compatibility with earlier call sites.
#[allow(dead_code)]
pub fn merge_configs(global: &GlobalConfig, project: &ProjectConfig) -> MergedConfig {
    let mut acc = RawSettings::default();
    let mut sources = SourceMap::new();
    acc.merge_from(global.clone(), SettingsSource::User, &mut sources);
    acc.merge_from(project.clone(), SettingsSource::Project, &mut sources);
    let mut merged = EffectiveSettings::from_raw(acc);
    apply_env_overrides(&mut merged, &mut sources);
    merged
}

/// Apply environment-variable overrides in place.
fn apply_env_overrides(merged: &mut EffectiveSettings, sources: &mut SourceMap) {
    let set_src = |key: &str, sources: &mut SourceMap| {
        sources.insert(key.to_string(), SettingsSource::Env);
    };

    if let Ok(model) = std::env::var("CLAUDE_MODEL") {
        merged.model = Some(model);
        set_src("model", sources);
    }
    if let Ok(backend) = std::env::var("CC_BACKEND").or_else(|_| std::env::var("CLAUDE_BACKEND")) {
        merged.backend = Some(backend);
        set_src("backend", sources);
    }
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        merged.api_key = Some(key);
        set_src("apiKey", sources);
    }
    if let Ok(v) = std::env::var("CLAUDE_VERBOSE") {
        merged.verbose = v == "1" || v.eq_ignore_ascii_case("true");
        set_src("verbose", sources);
    }
    if let Ok(mode) = std::env::var("CLAUDE_PERMISSION_MODE") {
        merged.permission_mode = Some(mode.clone());
        merged.permissions.default_mode = Some(mode);
        set_src("permissionMode", sources);
    }
    if let Ok(lang) = std::env::var("CLAUDE_LANGUAGE") {
        merged.language = Some(lang);
        set_src("language", sources);
    }
    if let Ok(style) = std::env::var("CLAUDE_OUTPUT_STYLE") {
        merged.output_style = Some(style);
        set_src("outputStyle", sources);
    }
    if let Ok(theme) = std::env::var("CLAUDE_THEME") {
        merged.theme = Some(theme);
        set_src("theme", sources);
    }
}

/// Load the full four-layer stack (managed/user/project/local) plus env.
///
/// This is the preferred entry point for new code. The legacy
/// [`load_and_merge`] wraps this and returns only [`EffectiveSettings`].
pub fn load_effective(cwd: &Path) -> Result<LoadedSettings> {
    let mut acc = RawSettings::default();
    let mut sources = SourceMap::new();
    let mut loaded_paths = Vec::new();
    let mut managed = None;
    let mut user = None;
    let mut project = None;
    let mut local = None;

    // 1. managed
    let managed_path = managed_settings_path();
    if let Some(raw) = load_raw_from(&managed_path)? {
        acc.merge_from(raw.clone(), SettingsSource::Managed, &mut sources);
        managed = Some(raw);
        loaded_paths.push((SettingsSource::Managed, managed_path));
    }

    // 2. user
    let user_path = user_settings_path();
    if let Some(raw) = load_raw_from(&user_path)? {
        acc.merge_from(raw.clone(), SettingsSource::User, &mut sources);
        user = Some(raw);
        loaded_paths.push((SettingsSource::User, user_path));
    }

    // 3. project
    if let Some(p) = find_project_config(cwd) {
        if let Some(raw) = load_raw_from(&p)? {
            acc.merge_from(raw.clone(), SettingsSource::Project, &mut sources);
            project = Some(raw);
            loaded_paths.push((SettingsSource::Project, p));
        }
    }

    // 4. local
    if let Some(p) = find_local_config(cwd) {
        if let Some(raw) = load_raw_from(&p)? {
            acc.merge_from(raw.clone(), SettingsSource::Local, &mut sources);
            local = Some(raw);
            loaded_paths.push((SettingsSource::Local, p));
        }
    }

    let mut effective = EffectiveSettings::from_raw(acc);

    // 5. env
    apply_env_overrides(&mut effective, &mut sources);

    Ok(LoadedSettings {
        effective,
        sources,
        managed,
        user,
        project,
        local,
        loaded_paths,
    })
}

/// Convenience wrapper — loads the full stack and returns the merged
/// runtime view.
pub fn load_and_merge(cwd: &str) -> Result<MergedConfig> {
    Ok(load_effective(Path::new(cwd))?.effective)
}

// ---------------------------------------------------------------------------
// Write + backup
// ---------------------------------------------------------------------------

/// Maximum number of backup copies to retain per settings file.
pub const MAX_SETTINGS_BACKUPS: usize = 5;

/// Serialise `raw` to JSON (pretty) with atomic write + rotating backup.
///
/// - Creates parent directories as needed.
/// - If `path` already exists, it's copied to `{path}.{timestamp}.bak`.
/// - Old backups beyond [`MAX_SETTINGS_BACKUPS`] are pruned.
pub fn write_settings_file(path: &Path, raw: &RawSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory {}", parent.display()))?;
    }

    if path.exists() {
        let ts = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let bak = path.with_extension(format!("json.{}.bak", ts));
        if let Err(e) = std::fs::copy(path, &bak) {
            tracing::warn!(
                source = %path.display(),
                target = %bak.display(),
                error = %e,
                "failed to copy settings backup"
            );
        }
        prune_backups(path, MAX_SETTINGS_BACKUPS);
    }

    let pretty = serde_json::to_string_pretty(raw)
        .context("Failed to serialize settings to JSON")?;

    // Atomic-ish: write to a tmp sibling, then rename.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, pretty)
        .with_context(|| format!("Failed to write {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("Failed to rename {} -> {}", tmp.display(), path.display()))?;

    Ok(())
}

/// Write to the user-level settings file.
pub fn write_user_settings(raw: &RawSettings) -> Result<PathBuf> {
    let path = user_settings_path();
    write_settings_file(&path, raw)?;
    Ok(path)
}

/// Write to `cwd/.cc-rust/settings.json`, creating the directory if needed.
pub fn write_project_settings(cwd: &Path, raw: &RawSettings) -> Result<PathBuf> {
    let dir = find_project_dir(cwd).unwrap_or_else(|| cwd.join(".cc-rust"));
    let path = dir.join("settings.json");
    write_settings_file(&path, raw)?;
    Ok(path)
}

/// Write to `cwd/.cc-rust/settings.local.json`.
pub fn write_local_settings(cwd: &Path, raw: &RawSettings) -> Result<PathBuf> {
    let dir = find_project_dir(cwd).unwrap_or_else(|| cwd.join(".cc-rust"));
    let path = dir.join("settings.local.json");
    write_settings_file(&path, raw)?;
    Ok(path)
}

fn prune_backups(path: &Path, keep: usize) {
    let Some(parent) = path.parent() else { return };
    let Some(stem) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return;
    };
    let prefix = format!("{}.", stem);

    let mut backups: Vec<PathBuf> = Vec::new();
    let Ok(entries) = std::fs::read_dir(parent) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name.ends_with(".bak") {
            backups.push(entry.path());
        }
    }
    // Sort newest-first by filename (our timestamp format is sortable).
    backups.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    for old in backups.into_iter().skip(keep) {
        let _ = std::fs::remove_file(old);
    }
}

// ---------------------------------------------------------------------------
// JSON Schema
// ---------------------------------------------------------------------------

/// Return a JSON Schema (Draft 2020-12) describing the on-disk
/// `settings.json` shape.
///
/// The schema is hand-maintained so the repo can commit it without pulling
/// in `schemars`. Keep in sync with [`RawSettings`].
pub fn settings_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://cc-rust/settings.schema.json",
        "title": "cc-rust settings",
        "description": "On-disk shape of settings.json (managed/user/project/local).",
        "type": "object",
        "additionalProperties": true,
        "properties": {
            "model": { "type": "string" },
            "backend": { "type": "string", "enum": ["native", "codex"] },
            "theme": { "type": "string" },
            "verbose": { "type": "boolean" },
            "permissionMode": {
                "type": "string",
                "enum": ["default", "ask", "auto", "bypass", "plan", "acceptEdits", "dontAsk"]
            },
            "allowedTools": { "type": "array", "items": { "type": "string" } },
            "permissions": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "defaultMode": {
                        "type": "string",
                        "enum": ["default", "ask", "auto", "bypass", "plan", "acceptEdits", "dontAsk"]
                    },
                    "allow": { "type": "array", "items": { "type": "string" } },
                    "ask": { "type": "array", "items": { "type": "string" } },
                    "deny": { "type": "array", "items": { "type": "string" } },
                    "additionalDirectories": {
                        "type": "array", "items": { "type": "string" }
                    },
                    "enableBypassMode": { "type": "boolean" },
                    "enableAutoMode": { "type": "boolean" }
                }
            },
            "sandbox": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "enabled": { "type": "boolean" },
                    "mode": { "type": "string" },
                    "allowedCommands": {
                        "type": "array", "items": { "type": "string" }
                    }
                }
            },
            "hooks": { "type": "object", "additionalProperties": true },
            "statusLine": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "type": { "type": "string" },
                    "command": { "type": "string" },
                    "script": { "type": "string" },
                    "format": { "type": "string" }
                }
            },
            "outputStyle": { "type": "string" },
            "language": { "type": "string" },
            "voiceEnabled": { "type": "boolean" },
            "editorMode": { "type": "string", "enum": ["normal", "vim"] },
            "viewMode": { "type": "string" },
            "spinnerTips": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "enabled": { "type": "boolean" },
                    "intervalMs": { "type": "integer", "minimum": 0 },
                    "customTips": {
                        "type": "array", "items": { "type": "string" }
                    }
                }
            },
            "terminalProgressBarEnabled": { "type": "boolean" },
            "availableModels": {
                "type": "array", "items": { "type": "string" }
            },
            "effortLevel": { "type": "string" },
            "fastMode": { "type": "boolean" },
            "fastModePerSessionOptIn": { "type": "boolean" },
            "teammateMode": { "type": "boolean" },
            "claudeInChromeDefaultEnabled": { "type": "boolean" },
            "systemPrompt": { "type": "string" },
            "apiKey": { "type": "string" }
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_overrides_global() {
        let global = RawSettings {
            model: Some("claude-sonnet".into()),
            backend: Some("native".into()),
            theme: Some("dark".into()),
            verbose: Some(false),
            ..Default::default()
        };
        let project = RawSettings {
            model: Some("claude-opus".into()),
            backend: Some("codex".into()),
            verbose: Some(true),
            claude_in_chrome_default_enabled: Some(true),
            ..Default::default()
        };

        let merged = merge_configs(&global, &project);
        assert_eq!(merged.model.as_deref(), Some("claude-opus"));
        assert_eq!(merged.backend.as_deref(), Some("codex"));
        assert_eq!(merged.theme.as_deref(), Some("dark"));
        assert!(merged.verbose);
        assert_eq!(merged.claude_in_chrome_default_enabled, Some(true));
    }

    #[test]
    fn allowed_tools_dedup_merges() {
        let base: Vec<String> = vec!["Bash".into(), "FileRead".into()];
        let over: Vec<String> = vec!["FileRead".into(), "Grep".into()];
        let tools = merge_str_lists(Some(&base), Some(&over));
        assert_eq!(tools, vec!["Bash", "FileRead", "Grep"]);
    }

    #[test]
    fn empty_configs_produce_defaults() {
        let merged = merge_configs(&RawSettings::default(), &RawSettings::default());
        assert!(merged.model.is_none());
        assert!(merged.backend.is_none());
        assert!(!merged.verbose);
        assert!(merged.allowed_tools.is_empty());
    }

    #[test]
    fn permissions_legacy_fallback() {
        let raw = RawSettings {
            permission_mode: Some("auto".into()),
            allowed_tools: Some(vec!["Bash".into()]),
            ..Default::default()
        };
        let eff = EffectiveSettings::from_raw(raw);
        assert_eq!(eff.permissions.default_mode.as_deref(), Some("auto"));
        assert_eq!(eff.permissions.allow, vec!["Bash".to_string()]);
        assert_eq!(eff.permission_mode.as_deref(), Some("auto"));
    }

    #[test]
    fn permissions_nested_overrides_legacy() {
        let raw = RawSettings {
            permission_mode: Some("auto".into()),
            permissions: Some(PermissionsSettings {
                default_mode: Some("bypass".into()),
                allow: vec!["Grep".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let eff = EffectiveSettings::from_raw(raw);
        assert_eq!(eff.permissions.default_mode.as_deref(), Some("bypass"));
        assert!(eff.permissions.allow.contains(&"Grep".to_string()));
    }

    #[test]
    fn source_map_tracks_layer() {
        let user = RawSettings {
            model: Some("sonnet".into()),
            ..Default::default()
        };
        let project = RawSettings {
            model: Some("opus".into()),
            theme: Some("dark".into()),
            ..Default::default()
        };
        let mut acc = RawSettings::default();
        let mut sources = SourceMap::new();
        acc.merge_from(user, SettingsSource::User, &mut sources);
        acc.merge_from(project, SettingsSource::Project, &mut sources);
        assert_eq!(sources.get("model"), Some(&SettingsSource::Project));
        assert_eq!(sources.get("theme"), Some(&SettingsSource::Project));
    }

    #[test]
    fn unknown_keys_land_in_extra() {
        let raw: RawSettings = serde_json::from_str(
            r#"{ "model": "opus", "customFlag": true, "anotherNested": {"a":1} }"#,
        )
        .unwrap();
        assert_eq!(raw.model.as_deref(), Some("opus"));
        assert!(raw.extra.contains_key("customFlag"));
        assert!(raw.extra.contains_key("anotherNested"));
    }

    #[test]
    fn schema_has_known_keys() {
        let s = settings_schema();
        let props = s
            .pointer("/properties")
            .and_then(|v| v.as_object())
            .expect("schema has /properties");
        for key in [
            "model",
            "backend",
            "permissions",
            "sandbox",
            "statusLine",
            "outputStyle",
            "spinnerTips",
            "availableModels",
            "fastMode",
        ] {
            assert!(props.contains_key(key), "missing schema key: {}", key);
        }
    }

    /// The committed schema file is the canonical doc. This test makes
    /// sure it never drifts from the runtime [`settings_schema`] output.
    /// To regenerate the file, run:
    /// `cargo test schema_file_matches_runtime -- --ignored` (then update).
    #[test]
    fn schema_file_matches_runtime() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("docs")
            .join("schemas")
            .join("settings.schema.json");

        let on_disk: Value = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("docs/schemas/settings.schema.json missing"),
        )
        .expect("schema file is not valid JSON");

        let runtime = settings_schema();

        // Compare the `properties` object specifically — the human-curated
        // file may carry additional doc-only metadata but its property
        // shape must match. (We only assert the property keys are equal.)
        let on_disk_keys: std::collections::BTreeSet<_> = on_disk
            .pointer("/properties")
            .and_then(|v| v.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let runtime_keys: std::collections::BTreeSet<_> = runtime
            .pointer("/properties")
            .and_then(|v| v.as_object())
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        assert_eq!(
            on_disk_keys, runtime_keys,
            "settings.schema.json drift — committed file is missing keys \
             present in settings_schema(), or vice versa. Update \
             docs/schemas/settings.schema.json to match.",
        );
    }

    #[test]
    fn write_creates_backup_and_prunes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("settings.json");

        let raw1 = RawSettings {
            model: Some("claude-a".into()),
            ..Default::default()
        };
        write_settings_file(&path, &raw1).unwrap();
        assert!(path.exists());

        // Do several more writes separated by one second to get unique
        // backup timestamps (format is YYYYMMDD-HHMMSS).
        for i in 0..(MAX_SETTINGS_BACKUPS + 2) {
            std::thread::sleep(std::time::Duration::from_millis(1100));
            let raw = RawSettings {
                model: Some(format!("claude-{}", i)),
                ..Default::default()
            };
            write_settings_file(&path, &raw).unwrap();
        }

        let entries: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with("settings.json.") && n.ends_with(".bak")
            })
            .collect();
        assert!(
            entries.len() <= MAX_SETTINGS_BACKUPS,
            "too many backups: {}",
            entries.len()
        );
    }
}

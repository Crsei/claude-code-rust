//! CLI → runtime config assembly (Phase B helpers).
//!
//! Extracted from `main.rs`: small pieces of logic that map CLI/config
//! layers onto the long-lived runtime surfaces — working directory,
//! permission mode, Chrome enablement, tool permission context.

use std::collections::HashMap;

use crate::cli::Cli;
use crate::config::settings;
use crate::types::tool::{PermissionMode, ToolPermissionContext};

/// Resolve the working directory from CLI args or the current process cwd.
pub fn resolve_cwd(cli: &Cli) -> String {
    cli.cwd.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| ".".to_string())
    })
}

/// Map `--chrome` / `--no-chrome` flags onto an explicit Option<bool>.
/// `None` means "defer to config / env defaults".
pub fn chrome_cli_override(cli: &Cli) -> Option<bool> {
    if cli.chrome {
        Some(true)
    } else if cli.no_chrome {
        Some(false)
    } else {
        None
    }
}

/// True iff Chrome integration is enabled after layering CLI over config.
pub fn chrome_requested(cli: &Cli, config_default: Option<bool>) -> bool {
    matches!(
        crate::browser::session::resolve_enablement(chrome_cli_override(cli), config_default),
        crate::browser::session::ChromeEnablement::Enabled
    )
}

/// Resolve the permission mode from CLI arg or config.
pub fn resolve_permission_mode(
    cli_mode: Option<&str>,
    config_mode: Option<&str>,
) -> PermissionMode {
    let mode_str = cli_mode.or(config_mode).unwrap_or("default");
    PermissionMode::parse(mode_str)
}

/// Build the [`ToolPermissionContext`] from layered settings.
///
/// Pulls allow/ask/deny lists from `EffectiveSettings::permissions` and
/// folds them into per-source rule maps tagged with the source of the
/// settings layer that provided them. `enableBypassMode` /
/// `enableAutoMode` from settings gate the corresponding modes at runtime.
pub fn build_tool_permission_context(
    mode: PermissionMode,
    loaded: &settings::LoadedSettings,
) -> ToolPermissionContext {
    use settings::SettingsSource;

    let merged = &loaded.effective;

    let mut always_allow_rules: HashMap<String, Vec<String>> = HashMap::new();
    let mut always_deny_rules: HashMap<String, Vec<String>> = HashMap::new();
    let mut always_ask_rules: HashMap<String, Vec<String>> = HashMap::new();
    let mut additional_working_directories = HashMap::new();

    let push_rules =
        |map: &mut HashMap<String, Vec<String>>, source: SettingsSource, items: &[String]| {
            if items.is_empty() {
                return;
            }
            map.entry(source.as_str().to_string())
                .or_default()
                .extend(items.iter().cloned());
        };

    // Iterate lowest -> highest priority so /permissions show prints them
    // in a stable order; the matcher itself treats sources uniformly.
    let layers: [(
        SettingsSource,
        Option<&crate::config::settings::RawSettings>,
    ); 4] = [
        (SettingsSource::Managed, loaded.managed.as_ref()),
        (SettingsSource::User, loaded.user.as_ref()),
        (SettingsSource::Project, loaded.project.as_ref()),
        (SettingsSource::Local, loaded.local.as_ref()),
    ];

    for (src, raw_opt) in layers {
        let Some(raw) = raw_opt else { continue };
        if let Some(perms) = raw.permissions.as_ref() {
            push_rules(&mut always_allow_rules, src, &perms.allow);
            push_rules(&mut always_deny_rules, src, &perms.deny);
            push_rules(&mut always_ask_rules, src, &perms.ask);
            for dir in &perms.additional_directories {
                additional_working_directories.insert(
                    dir.clone(),
                    crate::types::tool::AdditionalWorkingDirectory {
                        path: dir.clone(),
                        read_only: false,
                    },
                );
            }
        }
        if let Some(legacy) = raw.allowed_tools.as_ref() {
            push_rules(&mut always_allow_rules, src, legacy);
        }
    }

    ToolPermissionContext {
        mode,
        additional_working_directories,
        always_allow_rules,
        always_deny_rules,
        always_ask_rules,
        session_allow_rules: HashMap::new(),
        is_bypass_permissions_mode_available: merged.permissions.enable_bypass_mode.unwrap_or(true),
        is_auto_mode_available: Some(merged.permissions.enable_auto_mode.unwrap_or(true)),
        pre_plan_mode: None,
    }
}

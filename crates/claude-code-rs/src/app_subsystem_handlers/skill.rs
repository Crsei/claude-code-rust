use cc_ipc_protocol::subsystem_events::{SkillCommand, SkillEvent};
use cc_ipc_protocol::BackendMessage;

use super::snapshot::{build_skill_info_list, discover_plugin_skills_for_handlers};

/// Handle a skill subsystem command from the frontend.
///
/// `Reload` clears and re-initialises the skill registry.
/// `QueryStatus` returns the full skill list.
pub fn handle_skill_command(cmd: SkillCommand) -> Vec<BackendMessage> {
    match cmd {
        SkillCommand::Reload => {
            let cwd = std::env::current_dir().ok();
            let plugin_skills = discover_plugin_skills_for_handlers();
            let report = cc_skills::reload_skills_with_extra(
                &cc_config::paths::skills_dir_global(),
                cwd.as_deref(),
                plugin_skills,
                cc_skills::SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
            );
            tracing::info!(
                count = report.loaded,
                skipped = report.skipped,
                revision = report.revision,
                errors = report.error_count(),
                warnings = report.warning_count(),
                "Skills reloaded via IPC"
            );
            let mut messages = vec![BackendMessage::SkillEvent {
                event: SkillEvent::SkillsLoaded {
                    count: report.loaded,
                },
            }];
            if report.error_count() > 0 || report.warning_count() > 0 {
                messages.push(BackendMessage::SystemInfo {
                    text: format!(
                        "Skill reload completed with {} warning(s), {} error(s).",
                        report.warning_count(),
                        report.error_count()
                    ),
                    level: if report.error_count() > 0 {
                        "warn".to_string()
                    } else {
                        "info".to_string()
                    },
                });
            }
            messages
        }
        SkillCommand::QueryStatus => {
            let skills = build_skill_info_list();
            vec![BackendMessage::SkillEvent {
                event: SkillEvent::SkillList { skills },
            }]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_skill_query_status_returns_skill_list() {
        let msgs = handle_skill_command(SkillCommand::QueryStatus);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(&msgs[0], BackendMessage::SkillEvent { .. }));
    }

    #[test]
    #[serial_test::serial]
    fn build_skill_info_list_returns_skills() {
        cc_skills::clear_skills();
        cc_skills::register_skill(cc_skills::SkillDefinition {
            name: "test-skill".to_string(),
            source: cc_skills::SkillSource::Bundled,
            base_dir: None,
            frontmatter: cc_skills::SkillFrontmatter {
                description: "A test".to_string(),
                user_invocable: true,
                ..Default::default()
            },
            prompt_body: String::new(),
        });
        let infos = super::super::snapshot::build_skill_info_list();
        let test = infos.iter().find(|s| s.name == "test-skill");
        assert!(test.is_some());
        assert_eq!(test.unwrap().source, "bundled");
        assert!(test.unwrap().user_invocable);
        cc_skills::clear_skills();
    }

    #[test]
    #[serial_test::serial]
    fn build_skill_info_list_maps_sources() {
        cc_skills::clear_skills();

        let sources = vec![
            ("bundled-sk", cc_skills::SkillSource::Bundled, "bundled"),
            ("user-sk", cc_skills::SkillSource::User, "user"),
            ("project-sk", cc_skills::SkillSource::Project, "project"),
            (
                "plugin-sk",
                cc_skills::SkillSource::Plugin("p".to_string()),
                "plugin",
            ),
            (
                "mcp-sk",
                cc_skills::SkillSource::Mcp("m".to_string()),
                "mcp",
            ),
        ];

        for (name, source, _) in &sources {
            cc_skills::register_skill(cc_skills::SkillDefinition {
                name: name.to_string(),
                source: source.clone(),
                base_dir: None,
                frontmatter: cc_skills::SkillFrontmatter {
                    description: "test".to_string(),
                    user_invocable: true,
                    ..Default::default()
                },
                prompt_body: String::new(),
            });
        }

        let infos = super::super::snapshot::build_skill_info_list();
        for (name, _, expected_source) in &sources {
            let info = infos.iter().find(|s| s.name == *name);
            assert!(info.is_some(), "skill {} should be present", name);
            assert_eq!(info.unwrap().source, *expected_source);
        }

        cc_skills::clear_skills();
    }
}

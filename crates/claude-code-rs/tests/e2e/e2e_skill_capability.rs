//! Skill discovery capability tests for project and plugin skill sources.

mod capability_lab_support;

use capability_lab_support::CapabilityLab;
use cc_skills::{SkillLoadOptions, SkillSource};
use serial_test::serial;

#[test]
#[serial]
fn project_skill_is_discovered_with_reference_files_in_cc_rust_project_dir() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    cc_skills::clear_skills();

    let report = cc_skills::reload_skills_with_extra(
        &lab.cc_rust_home.join("skills"),
        Some(&lab.project_dir),
        Vec::new(),
        SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
    );

    assert!(report.loaded >= 1, "expected project skill load report");
    let skills = cc_skills::get_all_skills();
    let product = skills
        .iter()
        .find(|skill| skill.name == "product-brief-writer")
        .expect("project product brief skill");
    assert_eq!(product.source, SkillSource::Project);
    assert!(
        product.frontmatter.description.contains("product briefs"),
        "description should drive model-trigger matching"
    );
    assert!(product
        .base_dir
        .as_ref()
        .unwrap()
        .join("references/style-guide.md")
        .is_file());

    cc_skills::clear_skills();
    lab.assert_path_isolated();
}

#[test]
#[serial]
fn project_skill_negative_prompt_has_no_hard_reference_preload() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    cc_skills::clear_skills();
    cc_skills::reload_skills_with_extra(
        &lab.cc_rust_home.join("skills"),
        Some(&lab.project_dir),
        Vec::new(),
        SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
    );

    let listing = cc_skills::invocation::build_model_skills_listing();
    assert!(listing.contains("product-brief-writer"));
    assert!(
        !listing.contains("Use direct headings"),
        "model skill listing should not preload reference file contents"
    );

    cc_skills::clear_skills();
}

#[test]
#[serial]
fn plugin_skill_is_loaded_only_when_plugin_contributes_it() {
    let lab = CapabilityLab::new();
    let (_home, _cc_home) = lab.set_env();
    cc_skills::clear_skills();

    let plugin_skill_path = lab
        .write_plugin_fixture()
        .join("skills")
        .join("plugin-review")
        .join("SKILL.md");
    let mut plugin_skill = cc_skills::loader::load_skill_from_file_path(
        &plugin_skill_path,
        SkillSource::Plugin("capability-plugin@local".to_string()),
    )
    .expect("load plugin skill fixture");
    plugin_skill.frontmatter.description = "Review capability lab changes.".to_string();

    cc_skills::reload_skills_with_extra(
        &lab.cc_rust_home.join("skills"),
        Some(&lab.project_dir),
        vec![plugin_skill],
        SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
    );

    let plugin = cc_skills::get_all_skills()
        .into_iter()
        .find(|skill| skill.name == "plugin-review")
        .expect("plugin skill should be registered");
    assert_eq!(
        plugin.source,
        SkillSource::Plugin("capability-plugin@local".to_string())
    );

    cc_skills::reload_skills_with_extra(
        &lab.cc_rust_home.join("skills"),
        Some(&lab.project_dir),
        Vec::new(),
        SkillLoadOptions::for_app_version(env!("CARGO_PKG_VERSION")),
    );
    assert!(
        cc_skills::get_all_skills()
            .into_iter()
            .all(|skill| skill.name != "plugin-review"),
        "plugin skill should disappear when plugin contributions are absent"
    );

    cc_skills::clear_skills();
}

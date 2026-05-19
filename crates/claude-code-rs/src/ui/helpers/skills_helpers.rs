//! Skill display and matching helpers.

use cc_skills::SkillDefinition;

#[cfg(test)]
pub(crate) const SKILL_NAME_TRUNCATE_LEN: usize = 21;

pub(crate) fn skill_display_name(skill: &SkillDefinition) -> &str {
    skill.display_name()
}

pub(crate) fn skill_description(skill: &SkillDefinition) -> &str {
    if let Some(when_to_use) = skill.frontmatter.when_to_use.as_deref() {
        if !when_to_use.trim().is_empty() {
            return when_to_use;
        }
    }
    skill.frontmatter.description.as_str()
}

#[cfg(test)]
pub(crate) fn truncate_skill_name(name: &str) -> String {
    let count = name.chars().count();
    if count <= SKILL_NAME_TRUNCATE_LEN {
        return name.to_string();
    }
    let mut out = name
        .chars()
        .take(SKILL_NAME_TRUNCATE_LEN.saturating_sub(1))
        .collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
pub(crate) fn match_skill(skill: &SkillDefinition, query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return true;
    }
    let haystacks = [
        skill.name.as_str(),
        skill_display_name(skill),
        skill_description(skill),
    ];
    haystacks
        .iter()
        .any(|value| value.to_ascii_lowercase().contains(&query))
}

use std::path::Path;

use crate::commands;

use super::metadata::command_meta;
use super::CommandItem;

pub(super) fn filtered_commands(query: &str, cwd: &Path) -> Vec<CommandItem> {
    let mut items: Vec<CommandItem> = commands::get_all_commands()
        .into_iter()
        .filter(|cmd| {
            fuzzy_match(&cmd.name, query)
                || fuzzy_match(&cmd.description, query)
                || cmd.aliases.iter().any(|alias| fuzzy_match(alias, query))
        })
        .map(|cmd| {
            let meta = command_meta(&cmd.name, cwd);
            CommandItem {
                usage: meta.usage,
                examples: meta.examples,
                edit_targets: meta.edit_targets,
                name: cmd.name,
                aliases: cmd.aliases,
                description: cmd.description,
            }
        })
        .collect();
    items.sort_by(|a, b| score(&a.name, query).cmp(&score(&b.name, query)));
    items
}

pub(super) fn command_from_argument_input(input: &str, cwd: &Path) -> Option<CommandItem> {
    let without_slash = input.strip_prefix('/')?;
    if !without_slash.contains(char::is_whitespace) {
        return None;
    }

    let name = without_slash.split_whitespace().next()?;
    commands::get_all_commands()
        .into_iter()
        .find(|cmd| cmd.name == name || cmd.aliases.iter().any(|alias| alias == name))
        .map(|cmd| {
            let meta = command_meta(&cmd.name, cwd);
            CommandItem {
                usage: meta.usage,
                examples: meta.examples,
                edit_targets: meta.edit_targets,
                name: cmd.name,
                aliases: cmd.aliases,
                description: cmd.description,
            }
        })
}

fn fuzzy_match(candidate: &str, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    let mut chars = candidate.chars();
    for q in query.chars() {
        if !chars.any(|c| c.eq_ignore_ascii_case(&q)) {
            return false;
        }
    }
    true
}

fn score(candidate: &str, query: &str) -> usize {
    if query.is_empty() {
        return 0;
    }
    if candidate == query {
        0
    } else if candidate.starts_with(query) {
        1
    } else if candidate.contains(query) {
        2
    } else {
        3
    }
}

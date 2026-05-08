use std::path::Path;

use crate::commands;
use crate::ui::fuzzy_match::best_fuzzy_match;

use super::metadata::command_meta;
use super::CommandItem;

pub(super) fn filtered_commands(query: &str, cwd: &Path) -> Vec<CommandItem> {
    let mut items: Vec<(usize, usize, CommandItem)> = commands::get_all_commands()
        .into_iter()
        .enumerate()
        .filter_map(|(index, cmd)| {
            let alias_candidates = cmd.aliases.iter().map(String::as_str);
            let matched = best_fuzzy_match(
                std::iter::once(cmd.name.as_str())
                    .chain(std::iter::once(cmd.description.as_str()))
                    .chain(alias_candidates),
                query,
            )?;
            let meta = command_meta(&cmd.name, cwd);
            Some((
                index,
                matched.score,
                CommandItem {
                    usage: meta.usage,
                    examples: meta.examples,
                    edit_targets: meta.edit_targets,
                    name: cmd.name,
                    aliases: cmd.aliases,
                    description: cmd.description,
                },
            ))
        })
        .collect();
    items.sort_by_key(|(index, score, _)| (*score, *index));
    items.into_iter().map(|(_, _, item)| item).collect()
}

fn command_item_for_name(name: &str, cwd: &Path) -> Option<CommandItem> {
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

pub(super) fn command_from_argument_input(input: &str, cwd: &Path) -> Option<CommandItem> {
    let without_slash = input.strip_prefix('/')?;
    if !without_slash.contains(char::is_whitespace) {
        return None;
    }

    let name = without_slash.split_whitespace().next()?;
    command_item_for_name(name, cwd)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::filtered_commands;

    #[test]
    fn command_filter_orders_exact_matches_before_subsequence_matches() {
        let commands = filtered_commands("cp", Path::new("/repo"));

        assert_eq!(
            commands.first().map(|command| command.name.as_str()),
            Some("copy")
        );
        assert!(commands.iter().any(|command| command.name == "compact"));
    }
}

//! Read-only shell command validation.
//!
//! Port of Bun's `readOnlyCommandValidation.ts` for classifying shell commands
//! as read-only or not. Used by Plan mode and Explore mode to decide whether
//! a Bash/PowerShell command can execute without user approval.
//!
//! # Design
//!
//! Each known command (git, gh, docker, rg, etc.) has a config of safe flags.
//! Unknown flags or positional arguments that could be dangerous cause the
//! command to be classified as NOT read-only.
//!
//! This is FAIL-CLOSED: any command we can't classify as read-only is treated
//! as potentially dangerous.

use std::collections::HashMap;

use cc_shell_command::fallback;
use cc_shell_command::model::{DiagnosticSeverity, ParseMode, ReadOnlyResult, ShellDialect};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Describes what kind of argument a flag expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagArgType {
    None,   // No argument (--color, -n)
    Number, // Integer argument (--context=3)
    String, // Any string argument (--relative=path)
    Char,   // Single character delimiter
    Brace,  // Literal "{}" only
}

/// Configuration for a single external command (e.g., "git diff").
#[derive(Debug, Clone)]
pub struct ExternalCommandConfig {
    /// Map of flag -> expected argument type. Only these flags are allowed.
    pub safe_flags: HashMap<String, FlagArgType>,
    /// Optional callback for additional danger checks.
    /// Returns true if the command is dangerous.
    pub additional_check: Option<fn(&str, &[String]) -> bool>,
    /// Whether the tool respects POSIX `--` end-of-options.
    /// Default: true.
    pub respects_double_dash: bool,
}

impl ExternalCommandConfig {
    pub fn new(safe_flags: HashMap<String, FlagArgType>) -> Self {
        Self {
            safe_flags,
            additional_check: None,
            respects_double_dash: true,
        }
    }

    pub fn with_check(
        safe_flags: HashMap<String, FlagArgType>,
        check: fn(&str, &[String]) -> bool,
    ) -> Self {
        Self {
            safe_flags,
            additional_check: Some(check),
            respects_double_dash: true,
        }
    }

    pub fn without_double_dash(mut self) -> Self {
        self.respects_double_dash = false;
        self
    }
}

// ---------------------------------------------------------------------------
// Utility helpers
// ---------------------------------------------------------------------------

fn map_from_pairs(pairs: &[(&str, FlagArgType)]) -> HashMap<String, FlagArgType> {
    pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
}

fn validate_flag_arg(value: &str, arg_type: FlagArgType) -> bool {
    match arg_type {
        FlagArgType::None => false, // Should not be called
        FlagArgType::Number => value.chars().all(|c| c.is_ascii_digit()),
        FlagArgType::String => true,
        FlagArgType::Char => value.chars().count() == 1,
        FlagArgType::Brace => value == "{}",
    }
}

// ---------------------------------------------------------------------------
// Git flag groups
// ---------------------------------------------------------------------------

fn git_ref_selection_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--all", FlagArgType::None),
        ("--branches", FlagArgType::None),
        ("--tags", FlagArgType::None),
        ("--remotes", FlagArgType::None),
    ])
}

fn git_date_filter_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--since", FlagArgType::String),
        ("--after", FlagArgType::String),
        ("--until", FlagArgType::String),
        ("--before", FlagArgType::String),
    ])
}

fn git_log_display_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--oneline", FlagArgType::None),
        ("--graph", FlagArgType::None),
        ("--decorate", FlagArgType::None),
        ("--no-decorate", FlagArgType::None),
        ("--date", FlagArgType::String),
        ("--relative-date", FlagArgType::None),
    ])
}

fn git_count_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--max-count", FlagArgType::Number),
        ("-n", FlagArgType::Number),
    ])
}

fn git_stat_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--stat", FlagArgType::None),
        ("--numstat", FlagArgType::None),
        ("--shortstat", FlagArgType::None),
        ("--name-only", FlagArgType::None),
        ("--name-status", FlagArgType::None),
    ])
}

fn git_color_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--color", FlagArgType::None),
        ("--no-color", FlagArgType::None),
    ])
}

fn git_patch_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--patch", FlagArgType::None),
        ("-p", FlagArgType::None),
        ("--no-patch", FlagArgType::None),
        ("--no-ext-diff", FlagArgType::None),
        ("-s", FlagArgType::None),
    ])
}

fn git_author_filter_flags() -> HashMap<String, FlagArgType> {
    map_from_pairs(&[
        ("--author", FlagArgType::String),
        ("--committer", FlagArgType::String),
        ("--grep", FlagArgType::String),
    ])
}

// ---------------------------------------------------------------------------
// GIT_READ_ONLY_COMMANDS
// ---------------------------------------------------------------------------

fn make_git_read_only_commands() -> HashMap<String, ExternalCommandConfig> {
    let mut m = HashMap::new();

    // git diff
    m.insert(
        "git diff".into(),
        ExternalCommandConfig::new(merge_maps(&[
            git_stat_flags(),
            git_color_flags(),
            map_from_pairs(&[
                ("--dirstat", FlagArgType::None),
                ("--summary", FlagArgType::None),
                ("--patch-with-stat", FlagArgType::None),
                ("--word-diff", FlagArgType::None),
                ("--word-diff-regex", FlagArgType::String),
                ("--color-words", FlagArgType::None),
                ("--no-renames", FlagArgType::None),
                ("--check", FlagArgType::None),
                ("--ws-error-highlight", FlagArgType::String),
                ("--full-index", FlagArgType::None),
                ("--binary", FlagArgType::None),
                ("--abbrev", FlagArgType::Number),
                ("--break-rewrites", FlagArgType::None),
                ("--find-renames", FlagArgType::None),
                ("--find-copies", FlagArgType::None),
                ("--find-copies-harder", FlagArgType::None),
                ("--irreversible-delete", FlagArgType::None),
                ("--diff-algorithm", FlagArgType::String),
                ("--histogram", FlagArgType::None),
                ("--patience", FlagArgType::None),
                ("--minimal", FlagArgType::None),
                ("--ignore-space-at-eol", FlagArgType::None),
                ("--ignore-space-change", FlagArgType::None),
                ("--ignore-all-space", FlagArgType::None),
                ("--ignore-blank-lines", FlagArgType::None),
                ("--inter-hunk-context", FlagArgType::Number),
                ("--function-context", FlagArgType::None),
                ("--exit-code", FlagArgType::None),
                ("--quiet", FlagArgType::None),
                ("--cached", FlagArgType::None),
                ("--staged", FlagArgType::None),
                ("--pickaxe-regex", FlagArgType::None),
                ("--pickaxe-all", FlagArgType::None),
                ("--no-index", FlagArgType::None),
                ("--relative", FlagArgType::String),
                ("--diff-filter", FlagArgType::String),
                ("-u", FlagArgType::None),
                ("-M", FlagArgType::None),
                ("-C", FlagArgType::None),
                ("-B", FlagArgType::None),
                ("-D", FlagArgType::None),
                ("-l", FlagArgType::Number),
                ("-S", FlagArgType::String),
                ("-G", FlagArgType::String),
                ("-O", FlagArgType::String),
                ("-R", FlagArgType::None),
            ]),
        ])),
    );

    // git log
    m.insert(
        "git log".into(),
        ExternalCommandConfig::new(merge_maps(&[
            git_log_display_flags(),
            git_ref_selection_flags(),
            git_date_filter_flags(),
            git_count_flags(),
            git_stat_flags(),
            git_color_flags(),
            git_patch_flags(),
            git_author_filter_flags(),
            map_from_pairs(&[
                ("--abbrev-commit", FlagArgType::None),
                ("--full-history", FlagArgType::None),
                ("--dense", FlagArgType::None),
                ("--sparse", FlagArgType::None),
                ("--simplify-merges", FlagArgType::None),
                ("--ancestry-path", FlagArgType::None),
                ("--source", FlagArgType::None),
                ("--first-parent", FlagArgType::None),
                ("--merges", FlagArgType::None),
                ("--no-merges", FlagArgType::None),
                ("--reverse", FlagArgType::None),
                ("--walk-reflogs", FlagArgType::None),
                ("--skip", FlagArgType::Number),
                ("--max-age", FlagArgType::Number),
                ("--min-age", FlagArgType::Number),
                ("--author-date-order", FlagArgType::None),
                ("--topo-order", FlagArgType::None),
                ("--date-order", FlagArgType::None),
                ("--follow", FlagArgType::None),
                ("--no-walk", FlagArgType::None),
                ("--left-right", FlagArgType::None),
                ("--cherry-mark", FlagArgType::None),
                ("--cherry-pick", FlagArgType::None),
                ("--boundary", FlagArgType::None),
                ("--pretty", FlagArgType::String),
                ("--format", FlagArgType::String),
                ("--diff-filter", FlagArgType::String),
                ("-S", FlagArgType::String),
                ("-G", FlagArgType::String),
            ]),
        ])),
    );

    // git show
    m.insert(
        "git show".into(),
        ExternalCommandConfig::new(merge_maps(&[
            git_log_display_flags(),
            git_stat_flags(),
            git_color_flags(),
            git_patch_flags(),
            map_from_pairs(&[
                ("--abbrev-commit", FlagArgType::None),
                ("--word-diff", FlagArgType::None),
                ("--word-diff-regex", FlagArgType::String),
                ("--color-words", FlagArgType::None),
                ("--pretty", FlagArgType::String),
                ("--format", FlagArgType::String),
                ("--first-parent", FlagArgType::None),
                ("--raw", FlagArgType::None),
                ("--diff-filter", FlagArgType::String),
                ("-m", FlagArgType::None),
                ("--quiet", FlagArgType::None),
            ]),
        ])),
    );

    // git shortlog
    m.insert(
        "git shortlog".into(),
        ExternalCommandConfig::new(merge_maps(&[
            git_ref_selection_flags(),
            git_date_filter_flags(),
            map_from_pairs(&[
                ("-s", FlagArgType::None),
                ("--summary", FlagArgType::None),
                ("-n", FlagArgType::None),
                ("--numbered", FlagArgType::None),
                ("-e", FlagArgType::None),
                ("--email", FlagArgType::None),
                ("-c", FlagArgType::None),
                ("--committer", FlagArgType::None),
                ("--group", FlagArgType::String),
                ("--format", FlagArgType::String),
                ("--no-merges", FlagArgType::None),
                ("--author", FlagArgType::String),
            ]),
        ])),
    );

    // git reflog
    m.insert(
        "git reflog".into(),
        ExternalCommandConfig::with_check(
            merge_maps(&[
                git_log_display_flags(),
                git_ref_selection_flags(),
                git_date_filter_flags(),
                git_count_flags(),
                git_author_filter_flags(),
            ]),
            |_raw: &str, args: &[String]| {
                let dangerous = ["expire", "delete", "exists"];
                for token in args {
                    if token.is_empty() || token.starts_with('-') {
                        continue;
                    }
                    if dangerous.contains(&token.as_str()) {
                        return true;
                    }
                    return false;
                }
                false
            },
        ),
    );

    // git stash list
    m.insert(
        "git stash list".into(),
        ExternalCommandConfig::new(merge_maps(&[
            git_log_display_flags(),
            git_ref_selection_flags(),
            git_count_flags(),
        ])),
    );

    // git ls-remote
    m.insert(
        "git ls-remote".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--branches", FlagArgType::None),
            ("-b", FlagArgType::None),
            ("--tags", FlagArgType::None),
            ("-t", FlagArgType::None),
            ("--heads", FlagArgType::None),
            ("--refs", FlagArgType::None),
            ("--quiet", FlagArgType::None),
            ("--sort", FlagArgType::String),
        ])),
    );

    // git status
    m.insert(
        "git status".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-s", FlagArgType::None),
            ("--short", FlagArgType::None),
            ("-b", FlagArgType::None),
            ("--branch", FlagArgType::None),
            ("--porcelain", FlagArgType::None),
            ("--long", FlagArgType::None),
            ("-v", FlagArgType::None),
            ("--verbose", FlagArgType::None),
            ("-u", FlagArgType::None),
            ("--untracked-files", FlagArgType::String),
            ("--ignore-submodules", FlagArgType::String),
            ("--column", FlagArgType::None),
            ("--no-column", FlagArgType::None),
            ("--renames", FlagArgType::None),
            ("--no-renames", FlagArgType::None),
        ])),
    );

    // git branch (read-only subcommands only)
    m.insert(
        "git branch".into(),
        ExternalCommandConfig::with_check(
            map_from_pairs(&[
                ("-a", FlagArgType::None),
                ("--all", FlagArgType::None),
                ("-r", FlagArgType::None),
                ("--remotes", FlagArgType::None),
                ("--list", FlagArgType::None),
                ("--show-current", FlagArgType::None),
                ("-v", FlagArgType::None),
                ("-vv", FlagArgType::None),
                ("--verbose", FlagArgType::None),
                ("--merged", FlagArgType::String),
                ("--no-merged", FlagArgType::String),
                ("--sort", FlagArgType::String),
                ("--format", FlagArgType::String),
                ("--color", FlagArgType::None),
                ("--no-color", FlagArgType::None),
                ("-q", FlagArgType::None),
                ("--quiet", FlagArgType::None),
            ]),
            |_raw: &str, args: &[String]| {
                // Block write subcommands: -d (delete), -D, -m (move), -M, -c (copy), -C
                // Block positional args that start with + (create)
                for token in args {
                    if token.is_empty() {
                        continue;
                    }
                    if token.starts_with('-') {
                        match token.as_str() {
                            "-d" | "-D" | "-m" | "-M" | "-c" | "-C" => return true,
                            _ => continue,
                        }
                    }
                    // First non-flag positional could be a branch name (safe to list)
                    // but also could be a creation: `git branch new-branch`
                    // We can't safely distinguish, so block.
                    return true;
                }
                false
            },
        ),
    );

    // git ls-files
    m.insert(
        "git ls-files".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-c", FlagArgType::None),
            ("--cached", FlagArgType::None),
            ("-m", FlagArgType::None),
            ("--modified", FlagArgType::None),
            ("-d", FlagArgType::None),
            ("--deleted", FlagArgType::None),
            ("-o", FlagArgType::None),
            ("--others", FlagArgType::None),
            ("-i", FlagArgType::None),
            ("--ignored", FlagArgType::None),
            ("-s", FlagArgType::None),
            ("--stage", FlagArgType::None),
            ("-u", FlagArgType::None),
            ("--unmerged", FlagArgType::None),
            ("-k", FlagArgType::None),
            ("--killed", FlagArgType::None),
            ("-z", FlagArgType::None),
            ("-t", FlagArgType::None),
            ("--exclude-standard", FlagArgType::None),
            ("--directory", FlagArgType::None),
            ("--no-empty-directory", FlagArgType::None),
            ("--full-name", FlagArgType::None),
            ("--recurse-submodules", FlagArgType::None),
            ("--error-unmatch", FlagArgType::None),
        ])),
    );

    // git describe
    m.insert(
        "git describe".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--all", FlagArgType::None),
            ("--tags", FlagArgType::None),
            ("--contains", FlagArgType::None),
            ("--abbrev", FlagArgType::Number),
            ("--candidates", FlagArgType::Number),
            ("--exact-match", FlagArgType::None),
            ("--debug", FlagArgType::None),
            ("--long", FlagArgType::None),
            ("--match", FlagArgType::String),
            ("--exclude", FlagArgType::String),
            ("--always", FlagArgType::None),
            ("--dirty", FlagArgType::None),
            ("--broken", FlagArgType::String),
        ])),
    );

    // git blame
    m.insert(
        "git blame".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-b", FlagArgType::None),
            ("--root", FlagArgType::None),
            ("--show-stats", FlagArgType::None),
            ("-L", FlagArgType::String),
            ("-l", FlagArgType::None),
            ("-t", FlagArgType::None),
            ("-s", FlagArgType::None),
            ("-w", FlagArgType::None),
            ("--line-porcelain", FlagArgType::None),
            ("--incremental", FlagArgType::None),
            ("--encoding", FlagArgType::String),
            ("--contents", FlagArgType::String),
            ("--date", FlagArgType::String),
            ("--ignore-rev", FlagArgType::String),
            ("--ignore-revs-file", FlagArgType::String),
            ("--color-lines", FlagArgType::None),
            ("--color-by-age", FlagArgType::None),
        ])),
    );

    // git grep
    m.insert(
        "git grep".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--cached", FlagArgType::None),
            ("--no-index", FlagArgType::None),
            ("--untracked", FlagArgType::None),
            ("--no-exclude-standard", FlagArgType::None),
            ("--exclude-standard", FlagArgType::None),
            ("--recurse-submodules", FlagArgType::None),
            ("--parent-basename", FlagArgType::None),
            ("-v", FlagArgType::None),
            ("--invert-match", FlagArgType::None),
            ("-i", FlagArgType::None),
            ("--ignore-case", FlagArgType::None),
            ("-w", FlagArgType::None),
            ("--word-regexp", FlagArgType::None),
            ("-E", FlagArgType::None),
            ("--extended-regexp", FlagArgType::None),
            ("-F", FlagArgType::None),
            ("--fixed-strings", FlagArgType::None),
            ("-G", FlagArgType::None),
            ("--basic-regexp", FlagArgType::None),
            ("-P", FlagArgType::None),
            ("--perl-regexp", FlagArgType::None),
            ("-n", FlagArgType::None),
            ("--line-number", FlagArgType::None),
            ("-h", FlagArgType::None),
            ("-o", FlagArgType::None),
            ("--only-matching", FlagArgType::None),
            ("--column", FlagArgType::None),
            ("-l", FlagArgType::None),
            ("--files-with-matches", FlagArgType::None),
            ("-L", FlagArgType::None),
            ("--files-without-match", FlagArgType::None),
            ("-q", FlagArgType::None),
            ("--quiet", FlagArgType::None),
            ("--count", FlagArgType::None),
            ("--all-match", FlagArgType::None),
            ("-A", FlagArgType::Number),
            ("--after-context", FlagArgType::Number),
            ("-B", FlagArgType::Number),
            ("--before-context", FlagArgType::Number),
            ("-C", FlagArgType::Number),
            ("--context", FlagArgType::Number),
            ("-W", FlagArgType::None),
            ("--funcname", FlagArgType::None),
            ("--function-context", FlagArgType::None),
            ("--max-count", FlagArgType::Number),
            ("--max-depth", FlagArgType::Number),
            ("--break", FlagArgType::None),
            ("--heading", FlagArgType::None),
            ("-p", FlagArgType::None),
            ("--show-function", FlagArgType::None),
            ("--color", FlagArgType::String),
            ("--no-color", FlagArgType::None),
            ("--color-matching", FlagArgType::None),
            ("--color-matched", FlagArgType::None),
            ("--no-full-name", FlagArgType::None),
            ("--full-name", FlagArgType::None),
            ("--threads", FlagArgType::Number),
            ("-f", FlagArgType::String),
            ("--file", FlagArgType::String),
            ("-e", FlagArgType::String),
            ("--and", FlagArgType::None),
            ("--or", FlagArgType::None),
            ("--not", FlagArgType::None),
            ("--open-files-in-pager", FlagArgType::None),
        ])),
    );

    // git config (read-only)
    m.insert(
        "git config".into(),
        ExternalCommandConfig::with_check(
            map_from_pairs(&[
                ("--list", FlagArgType::None),
                ("-l", FlagArgType::None),
                ("--global", FlagArgType::None),
                ("--system", FlagArgType::None),
                ("--local", FlagArgType::None),
                ("--worktree", FlagArgType::None),
                ("--file", FlagArgType::String),
                ("-f", FlagArgType::String),
                ("--blob", FlagArgType::String),
                ("--get", FlagArgType::None),
                ("--get-all", FlagArgType::None),
                ("--get-color", FlagArgType::None),
                ("--get-colorbool", FlagArgType::None),
                ("--get-urlmatch", FlagArgType::None),
                ("--bool", FlagArgType::None),
                ("--int", FlagArgType::None),
                ("--bool-or-int", FlagArgType::None),
                ("--path", FlagArgType::None),
                ("--type", FlagArgType::String),
                ("--show-origin", FlagArgType::None),
                ("--show-scope", FlagArgType::None),
                ("--name-only", FlagArgType::None),
                ("-z", FlagArgType::None),
                ("--null", FlagArgType::None),
                ("--includes", FlagArgType::None),
                ("--no-includes", FlagArgType::None),
                ("--default", FlagArgType::String),
                ("--comment", FlagArgType::String),
                ("--fixed-value", FlagArgType::None),
            ]),
            |_raw: &str, args: &[String]| {
                // Block if any positional looks like setting a value: `git config key value`
                // Read-only allows: `git config --get key`, `git config --list`, `git config key` (shows value)
                // Block: any positional after key name that doesn't start with -
                let mut positional_count = 0u32;
                for token in args {
                    if token.is_empty() || token.starts_with('-') {
                        continue;
                    }
                    positional_count += 1;
                    if positional_count > 1 {
                        return true; // Setting a value
                    }
                }
                false
            },
        ),
    );

    // git tag (read-only subcommands)
    m.insert(
        "git tag".into(),
        ExternalCommandConfig::with_check(
            map_from_pairs(&[
                ("-l", FlagArgType::None),
                ("--list", FlagArgType::None),
                ("-n", FlagArgType::Number),
                ("--sort", FlagArgType::String),
                ("--format", FlagArgType::String),
                ("--color", FlagArgType::None),
                ("--no-color", FlagArgType::None),
                ("--merged", FlagArgType::String),
                ("--no-merged", FlagArgType::String),
                ("--contains", FlagArgType::String),
                ("--no-contains", FlagArgType::String),
                ("--points-at", FlagArgType::String),
                ("--column", FlagArgType::None),
            ]),
            |_raw: &str, args: &[String]| {
                // Block if any positional looks like creating/deleting a tag
                for token in args {
                    if token.is_empty() || token.starts_with('-') {
                        continue;
                    }
                    // First non-flag could be a tag name to list, or to create.
                    // Without -l/--list, creation is implied. Block.
                    if !args.iter().any(|a| a == "-l" || a == "--list") {
                        return true;
                    }
                    return false;
                }
                false
            },
        ),
    );

    // git help
    m.insert(
        "git help".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-a", FlagArgType::None),
            ("--all", FlagArgType::None),
            ("-g", FlagArgType::None),
            ("--guides", FlagArgType::None),
            ("-i", FlagArgType::None),
            ("--info", FlagArgType::None),
            ("-m", FlagArgType::None),
            ("--man", FlagArgType::None),
            ("-w", FlagArgType::None),
            ("--web", FlagArgType::None),
        ])),
    );

    // git remote (read-only)
    m.insert(
        "git remote".into(),
        ExternalCommandConfig::with_check(
            map_from_pairs(&[("-v", FlagArgType::None), ("--verbose", FlagArgType::None)]),
            |_raw: &str, args: &[String]| {
                let write_subcommands = [
                    "add",
                    "rename",
                    "remove",
                    "set-head",
                    "set-branches",
                    "set-url",
                    "prune",
                    "update",
                ];
                for token in args {
                    if token.is_empty() || token.starts_with('-') {
                        continue;
                    }
                    if write_subcommands.contains(&token.as_str()) {
                        return true;
                    }
                    // `git remote` or `git remote -v` or `git remote show origin`
                    if token == "show" {
                        return false; // `show` is safe
                    }
                    return false; // unknown positional, could be safe (a remote name to show)
                }
                false
            },
        ),
    );

    m
}

// ---------------------------------------------------------------------------
// GH_READ_ONLY_COMMANDS
// ---------------------------------------------------------------------------

fn make_gh_read_only_commands() -> HashMap<String, ExternalCommandConfig> {
    let mut m = HashMap::new();

    // gh (root) — just `gh` with no subcommand is read-only (help)
    // Additional check blocks subcommand-like positional args so `gh pr merge` etc.
    // don't accidentally match the root entry.
    m.insert(
        "gh".into(),
        ExternalCommandConfig::with_check(map_from_pairs(&[]), |_raw: &str, args: &[String]| {
            // If there are non-flag positional args after bare "gh",
            // this is a subcommand, not just help — block it.
            args.iter().any(|a| !a.starts_with('-'))
        }),
    );

    // gh pr view
    m.insert(
        "gh pr view".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
            ("--comments", FlagArgType::None),
            ("-c", FlagArgType::None),
            ("--web", FlagArgType::None),
            ("-w", FlagArgType::None),
        ])),
    );

    // gh pr list
    m.insert(
        "gh pr list".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-a", FlagArgType::String),
            ("--assignee", FlagArgType::String),
            ("-l", FlagArgType::String),
            ("--label", FlagArgType::String),
            ("-L", FlagArgType::Number),
            ("--limit", FlagArgType::Number),
            ("-s", FlagArgType::String),
            ("--state", FlagArgType::String),
            ("--author", FlagArgType::String),
            ("--base", FlagArgType::String),
            ("--draft", FlagArgType::None),
            ("--search", FlagArgType::String),
            ("-q", FlagArgType::String),
            ("--jq", FlagArgType::String),
            ("--template", FlagArgType::String),
            ("--web", FlagArgType::None),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
        ])),
    );

    // gh issue list
    m.insert(
        "gh issue list".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-a", FlagArgType::String),
            ("--assignee", FlagArgType::String),
            ("-l", FlagArgType::String),
            ("--label", FlagArgType::String),
            ("-L", FlagArgType::Number),
            ("--limit", FlagArgType::Number),
            ("-s", FlagArgType::String),
            ("--state", FlagArgType::String),
            ("--author", FlagArgType::String),
            ("--search", FlagArgType::String),
            ("-q", FlagArgType::String),
            ("--jq", FlagArgType::String),
            ("--template", FlagArgType::String),
            ("--web", FlagArgType::None),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
        ])),
    );

    // gh issue view
    m.insert(
        "gh issue view".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
            ("--comments", FlagArgType::None),
            ("-c", FlagArgType::None),
            ("--web", FlagArgType::None),
            ("-w", FlagArgType::None),
        ])),
    );

    // gh run list
    m.insert(
        "gh run list".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-L", FlagArgType::Number),
            ("--limit", FlagArgType::Number),
            ("-w", FlagArgType::String),
            ("--workflow", FlagArgType::String),
            ("-b", FlagArgType::String),
            ("--branch", FlagArgType::String),
            ("-u", FlagArgType::String),
            ("--user", FlagArgType::String),
            ("-c", FlagArgType::String),
            ("--commit", FlagArgType::String),
            ("-e", FlagArgType::String),
            ("--event", FlagArgType::String),
            ("--status", FlagArgType::String),
            ("-q", FlagArgType::String),
            ("--jq", FlagArgType::String),
            ("--template", FlagArgType::String),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
        ])),
    );

    // gh run view
    m.insert(
        "gh run view".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
            ("--exit-status", FlagArgType::None),
            ("--log", FlagArgType::None),
            ("--log-failed", FlagArgType::None),
            ("-v", FlagArgType::None),
            ("--verbose", FlagArgType::None),
            ("-q", FlagArgType::String),
            ("--jq", FlagArgType::String),
            ("--template", FlagArgType::String),
            ("--attempt", FlagArgType::Number),
        ])),
    );

    // gh search prs
    m.insert(
        "gh search prs".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-q", FlagArgType::String),
            ("--jq", FlagArgType::String),
            ("--template", FlagArgType::String),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
        ])),
    );

    // gh search issues
    m.insert(
        "gh search issues".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("--json", FlagArgType::String),
            ("-q", FlagArgType::String),
            ("--jq", FlagArgType::String),
            ("--template", FlagArgType::String),
            ("-R", FlagArgType::String),
            ("--repo", FlagArgType::String),
        ])),
    );

    // gh api (GET only)
    m.insert(
        "gh api".into(),
        ExternalCommandConfig::with_check(
            map_from_pairs(&[
                ("-H", FlagArgType::String),
                ("--header", FlagArgType::String),
                ("-F", FlagArgType::String),
                ("--field", FlagArgType::String),
                ("-q", FlagArgType::String),
                ("--jq", FlagArgType::String),
                ("--template", FlagArgType::String),
                ("--paginate", FlagArgType::None),
                ("--include", FlagArgType::None),
                ("--cache", FlagArgType::Number),
                ("--silent", FlagArgType::None),
            ]),
            |_raw: &str, args: &[String]| {
                // Block if first positional is not a GET method
                // `gh api` without method defaults to GET
                for token in args {
                    if token.is_empty() {
                        continue;
                    }
                    if token.starts_with('-') {
                        continue;
                    }
                    // First positional: could be a URL or method
                    // Block POST, PUT, PATCH, DELETE
                    let upper = token.to_uppercase();
                    if upper == "POST" || upper == "PUT" || upper == "PATCH" || upper == "DELETE" {
                        return true;
                    }
                    return false; // URL or GET method is safe
                }
                false
            },
        ),
    );

    m
}

// ---------------------------------------------------------------------------
// DOCKER_READ_ONLY_COMMANDS
// ---------------------------------------------------------------------------

fn make_docker_read_only_commands() -> HashMap<String, ExternalCommandConfig> {
    let mut m = HashMap::new();

    m.insert(
        "docker ps".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-a", FlagArgType::None),
            ("--all", FlagArgType::None),
            ("-f", FlagArgType::String),
            ("--filter", FlagArgType::String),
            ("--format", FlagArgType::String),
            ("-n", FlagArgType::Number),
            ("--last", FlagArgType::Number),
            ("-l", FlagArgType::None),
            ("--latest", FlagArgType::None),
            ("-q", FlagArgType::None),
            ("--quiet", FlagArgType::None),
            ("-s", FlagArgType::None),
            ("--size", FlagArgType::None),
            ("--no-trunc", FlagArgType::None),
        ])),
    );

    m.insert(
        "docker images".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-a", FlagArgType::None),
            ("--all", FlagArgType::None),
            ("-f", FlagArgType::String),
            ("--filter", FlagArgType::String),
            ("--format", FlagArgType::String),
            ("-q", FlagArgType::None),
            ("--quiet", FlagArgType::None),
            ("--no-trunc", FlagArgType::None),
            ("--digests", FlagArgType::None),
            ("--tree", FlagArgType::None),
        ])),
    );

    m.insert(
        "docker inspect".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-f", FlagArgType::String),
            ("--format", FlagArgType::String),
            ("-s", FlagArgType::None),
            ("--size", FlagArgType::None),
            ("--type", FlagArgType::String),
        ])),
    );

    m.insert(
        "docker network ls".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-f", FlagArgType::String),
            ("--filter", FlagArgType::String),
            ("--format", FlagArgType::String),
            ("-q", FlagArgType::None),
            ("--quiet", FlagArgType::None),
            ("--no-trunc", FlagArgType::None),
        ])),
    );

    m.insert(
        "docker volume ls".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-f", FlagArgType::String),
            ("--filter", FlagArgType::String),
            ("--format", FlagArgType::String),
            ("-q", FlagArgType::None),
            ("--quiet", FlagArgType::None),
            ("--no-trunc", FlagArgType::None),
        ])),
    );

    m.insert(
        "docker logs".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-f", FlagArgType::None),
            ("--follow", FlagArgType::None),
            ("--tail", FlagArgType::String),
            ("-t", FlagArgType::None),
            ("--timestamps", FlagArgType::None),
            ("--details", FlagArgType::None),
            ("-n", FlagArgType::Number),
        ])),
    );

    m.insert(
        "docker info".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-f", FlagArgType::String),
            ("--format", FlagArgType::String),
        ])),
    );

    m.insert(
        "docker version".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-f", FlagArgType::String),
            ("--format", FlagArgType::String),
        ])),
    );

    m.insert(
        "docker stats".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-a", FlagArgType::None),
            ("--all", FlagArgType::None),
            ("--format", FlagArgType::String),
            ("--no-stream", FlagArgType::None),
            ("--no-trunc", FlagArgType::None),
        ])),
    );

    m
}

// ---------------------------------------------------------------------------
// RIPGREP_READ_ONLY_COMMANDS
// ---------------------------------------------------------------------------

fn make_rg_read_only_commands() -> HashMap<String, ExternalCommandConfig> {
    let mut m = HashMap::new();

    m.insert(
        "rg".into(),
        ExternalCommandConfig::new(map_from_pairs(&[
            ("-A", FlagArgType::Number),
            ("--after-context", FlagArgType::Number),
            ("-B", FlagArgType::Number),
            ("--before-context", FlagArgType::Number),
            ("-C", FlagArgType::Number),
            ("--context", FlagArgType::Number),
            ("--context-separator", FlagArgType::String),
            ("-c", FlagArgType::None),
            ("--count", FlagArgType::None),
            ("--count-matches", FlagArgType::None),
            ("--color", FlagArgType::String),
            ("--colors", FlagArgType::String),
            ("--column", FlagArgType::None),
            ("--crlf", FlagArgType::None),
            ("--debug", FlagArgType::None),
            ("--dfa-size-limit", FlagArgType::String),
            ("-E", FlagArgType::String),
            ("--encoding", FlagArgType::String),
            ("--engine", FlagArgType::String),
            ("-f", FlagArgType::String),
            ("--file", FlagArgType::String),
            ("--files", FlagArgType::None),
            ("--files-without-match", FlagArgType::None),
            ("-l", FlagArgType::None),
            ("--files-with-matches", FlagArgType::None),
            ("-F", FlagArgType::None),
            ("--fixed-strings", FlagArgType::None),
            ("-g", FlagArgType::String),
            ("--glob", FlagArgType::String),
            ("--glob-case-insensitive", FlagArgType::None),
            ("-G", FlagArgType::String),
            ("--iglob", FlagArgType::String),
            ("--heading", FlagArgType::None),
            ("--hidden", FlagArgType::None),
            ("-i", FlagArgType::None),
            ("--ignore-case", FlagArgType::None),
            ("--ignore-file", FlagArgType::String),
            ("--ignore-file-case-insensitive", FlagArgType::None),
            ("--include-zero", FlagArgType::None),
            ("-I", FlagArgType::None),
            ("--no-ignore", FlagArgType::None),
            ("--no-ignore-dot", FlagArgType::None),
            ("--no-ignore-exclude", FlagArgType::None),
            ("--no-ignore-files", FlagArgType::None),
            ("--no-ignore-global", FlagArgType::None),
            ("--no-ignore-messages", FlagArgType::None),
            ("--no-ignore-parent", FlagArgType::None),
            ("--no-ignore-vcs", FlagArgType::None),
            ("--json", FlagArgType::None),
            ("-x", FlagArgType::None),
            ("--line-regexp", FlagArgType::None),
            ("-N", FlagArgType::None),
            ("--no-line-number", FlagArgType::None),
            ("-n", FlagArgType::None),
            ("--line-number", FlagArgType::None),
            ("-m", FlagArgType::Number),
            ("--max-count", FlagArgType::Number),
            ("--max-columns", FlagArgType::Number),
            ("--max-columns-preview", FlagArgType::None),
            ("--max-depth", FlagArgType::Number),
            ("--max-filesize", FlagArgType::String),
            ("-M", FlagArgType::String),
            ("--multiline", FlagArgType::None),
            ("--multiline-dotall", FlagArgType::None),
            ("--no-unicode", FlagArgType::None),
            ("--no-require-git", FlagArgType::None),
            ("-0", FlagArgType::None),
            ("--null", FlagArgType::None),
            ("--one-file-system", FlagArgType::None),
            ("-o", FlagArgType::None),
            ("--only-matching", FlagArgType::None),
            ("--path-separator", FlagArgType::String),
            ("-p", FlagArgType::None),
            ("--pretty", FlagArgType::None),
            ("-P", FlagArgType::None),
            ("--pcre2", FlagArgType::None),
            ("--pcre2-version", FlagArgType::None),
            ("--pre", FlagArgType::String),
            ("--pre-glob", FlagArgType::String),
            ("-q", FlagArgType::None),
            ("--quiet", FlagArgType::None),
            ("--regex-size-limit", FlagArgType::String),
            ("-e", FlagArgType::String),
            ("--regexp", FlagArgType::String),
            ("-r", FlagArgType::String),
            ("--replace", FlagArgType::String),
            ("-s", FlagArgType::None),
            ("--case-sensitive", FlagArgType::None),
            ("-S", FlagArgType::None),
            ("--smart-case", FlagArgType::None),
            ("--sort", FlagArgType::String),
            ("--sortr", FlagArgType::String),
            ("--stats", FlagArgType::None),
            ("-a", FlagArgType::None),
            ("--text", FlagArgType::None),
            ("--threads", FlagArgType::Number),
            ("-t", FlagArgType::String),
            ("--type", FlagArgType::String),
            ("--type-list", FlagArgType::None),
            ("-T", FlagArgType::String),
            ("--type-not", FlagArgType::String),
            ("--type-add", FlagArgType::String),
            ("--type-clear", FlagArgType::String),
            ("-u", FlagArgType::None),
            ("--unrestricted", FlagArgType::None),
            ("-U", FlagArgType::None),
            ("--multiline", FlagArgType::None),
            ("-v", FlagArgType::None),
            ("--invert-match", FlagArgType::None),
            ("-w", FlagArgType::None),
            ("--word-regexp", FlagArgType::None),
            ("--no-messages", FlagArgType::None),
            ("--no-config", FlagArgType::None),
            ("--no-ignore", FlagArgType::None),
            ("--no-ignore-dot", FlagArgType::None),
            ("--no-ignore-exclude", FlagArgType::None),
            ("--no-ignore-files", FlagArgType::None),
            ("--no-ignore-global", FlagArgType::None),
            ("--no-ignore-messages", FlagArgType::None),
            ("--no-ignore-parent", FlagArgType::None),
            ("--no-ignore-vcs", FlagArgType::None),
            ("--no-unicode", FlagArgType::None),
        ])),
    );

    m
}

// ---------------------------------------------------------------------------
// PYRIGHT_READ_ONLY_COMMANDS
// ---------------------------------------------------------------------------

fn make_pyright_read_only_commands() -> HashMap<String, ExternalCommandConfig> {
    let mut m = HashMap::new();

    m.insert(
        "pyright".into(),
        ExternalCommandConfig::with_check(
            map_from_pairs(&[
                ("--version", FlagArgType::None),
                ("--help", FlagArgType::None),
                ("--project", FlagArgType::String),
                ("-p", FlagArgType::String),
                ("--pythonversion", FlagArgType::String),
                ("--pythonplatform", FlagArgType::String),
                ("--skipunused", FlagArgType::None),
                ("--level", FlagArgType::String),
                ("--outputjson", FlagArgType::None),
                ("--typeshedpath", FlagArgType::String),
                ("--venvpath", FlagArgType::String),
                ("--verbose", FlagArgType::None),
                ("--dependencies", FlagArgType::None),
                ("--warnings", FlagArgType::None),
                ("--verifytypes", FlagArgType::String),
                ("--stats", FlagArgType::None),
            ]),
            |_raw: &str, args: &[String]| args.iter().any(|arg| arg == "--watch" || arg == "-w"),
        )
        .without_double_dash(),
    );

    m
}

// ---------------------------------------------------------------------------
// EXTERNAL_READONLY_COMMANDS (cross-shell safe commands)
// ---------------------------------------------------------------------------

fn make_external_readonly_commands() -> HashMap<String, ExternalCommandConfig> {
    let mut m = HashMap::new();

    for cmd in &[
        "ls",
        "cat",
        "head",
        "tail",
        "echo",
        "printf",
        "which",
        "type",
        "pwd",
        "whoami",
        "id",
        "date",
        "uname",
        "env",
        "printenv",
        "true",
        "false",
        "yes",
        "dirname",
        "basename",
        "readlink",
        "realpath",
        "tty",
        "wc",
        "uniq",
        "cut",
        "tr",
        "fold",
        "nl",
        "od",
        "hexdump",
        "xxd",
        "strings",
        "df",
        "du",
        "stat",
        "lsblk",
        "blkid",
        "lscpu",
        "lsusb",
        "lspci",
        "arch",
        "nproc",
        "getconf",
        "mkvextract",
        "mediainfo",
        "ffprobe",
        "exiftool",
        "file",
        "mimetype",
        "jq",
        "yq",
        "mlr",
        "ps",
        "top",
        "htop",
        "btop",
        "uptime",
        "w",
        "lsof",
        "fuser",
        "ip",
        "ifconfig",
        "ss",
        "netstat",
        "route",
        "arp",
        "bat",
        "batcat",
        "delta",
        "diff",
        "diff3",
        "colordiff",
        "tree",
        "exa",
        "eza",
        "lsd",
        "rg",
        "ripgrep",
        "ag",
        "ack",
        "grep",
        "egrep",
        "fgrep",
        "pt",
        "tldr",
        "man",
        "whatis",
        "apropos",
    ] {
        m.insert(cmd.to_string(), ExternalCommandConfig::new(HashMap::new()));
    }

    m
}

// ---------------------------------------------------------------------------
// Flag validation
// ---------------------------------------------------------------------------

/// FLAG_PATTERN matches flag-like tokens.
/// A flag starts with `-` and has at least one more character.
fn is_flag(token: &str) -> bool {
    token.len() > 1 && token.starts_with('-')
}

/// Validate flags for a command against its config.
fn validate_flags(
    args: &[String],
    config: &ExternalCommandConfig,
    command_name: Option<&str>,
    raw_command: &str,
) -> bool {
    if let Some(check) = config.additional_check {
        if check(raw_command, args) {
            return false;
        }
    }

    let mut i = 0;

    while i < args.len() {
        let token = &args[i];

        if token.is_empty() {
            i += 1;
            continue;
        }

        // Handle `--` end-of-options
        if token == "--" {
            if config.respects_double_dash {
                break; // Everything after -- is positional args
            }
            i += 1;
            continue;
        }

        if is_flag(token) {
            // Handle --flag=value format
            let has_equals = token.contains('=');
            let parts: Vec<&str> = token.splitn(2, '=').collect();
            let flag = parts[0];
            let inline_value = parts.get(1).copied().unwrap_or("");

            // Get the flag arg type
            let flag_arg_type = config.safe_flags.get(flag).copied();

            match flag_arg_type {
                None => {
                    // Special case: git numeric shorthand (-<number>)
                    if command_name == Some("git")
                        && flag.len() > 1
                        && flag.chars().skip(1).all(|c| c.is_ascii_digit())
                    {
                        i += 1;
                        continue;
                    }
                    return false; // Unknown flag
                }
                Some(arg_type) => {
                    if arg_type == FlagArgType::None {
                        if has_equals {
                            return false; // Flag should not have a value
                        }
                        i += 1;
                    } else {
                        if has_equals {
                            // Use the inline value
                            if arg_type == FlagArgType::String
                                && inline_value.starts_with('-')
                                && !(command_name == Some("git")
                                    && flag == "--sort"
                                    && inline_value
                                        .chars()
                                        .nth(1)
                                        .map(|c| c.is_ascii_alphabetic())
                                        .unwrap_or(false))
                            {
                                return false;
                            }
                            if !validate_flag_arg(inline_value, arg_type) {
                                return false;
                            }
                            i += 1;
                        } else {
                            // Check if next token is the argument
                            if i + 1 < args.len() && !is_flag(&args[i + 1]) {
                                if arg_type == FlagArgType::String
                                    && args[i + 1].starts_with('-')
                                    && !(command_name == Some("git")
                                        && flag == "--sort"
                                        && args[i + 1]
                                            .chars()
                                            .nth(1)
                                            .map(|c| c.is_ascii_alphabetic())
                                            .unwrap_or(false))
                                {
                                    return false;
                                }
                                if !validate_flag_arg(&args[i + 1], arg_type) {
                                    return false;
                                }
                                i += 2;
                            } else {
                                return false; // Missing required argument
                            }
                        }
                    }
                }
            }
        } else {
            // Non-flag positional argument (revision specs, paths, patterns).
            // Keep scanning because later flag-looking tokens are still parsed by
            // the underlying command and may enable writes.
            i += 1;
        }
    }

    true
}

// ---------------------------------------------------------------------------
// UNC path vulnerability check
// ---------------------------------------------------------------------------

/// Check if a path contains a vulnerable UNC path that could leak credentials.
///
/// UNC paths like `\\host\share` can be used to leak NTLM hashes when
/// Windows attempts to authenticate with the remote host. This check
/// detects UNC paths in command arguments.
///
/// Port of Bun's `containsVulnerableUncPath`.
pub fn contains_vulnerable_unc_path(command: &str) -> bool {
    if !command.contains("\\\\") && !command.contains("//") {
        return false;
    }

    // Check for UNC patterns: \\host\share or //host/share anywhere in the line.
    // These can appear as arguments like `copy \\attacker\share\file.txt dest`.
    for line in command.lines() {
        let trimmed = line.trim();

        // Search for \\ anywhere in the line
        let mut search_start = 0;
        while let Some(pos) = trimmed[search_start..].find("\\\\") {
            let abs_pos = search_start + pos;
            let substr = &trimmed[abs_pos..];

            // Skip \\?\ extended-length paths and \\?\UNC\ paths
            if substr.starts_with("\\\\?\\") {
                search_start = abs_pos + 4;
                continue;
            }

            // Check for \\host\share pattern
            let after_slash = &substr[2..];
            if let Some(slash_pos) = after_slash.find('\\') {
                if slash_pos > 0 {
                    let host = &after_slash[..slash_pos];
                    let host_lower = host.to_lowercase();
                    if host_lower == "localhost"
                        || host_lower == "127.0.0.1"
                        || host_lower == "."
                        || host_lower == "?"
                    {
                        search_start = abs_pos + 2;
                        continue;
                    }
                    return true;
                }
            }
            search_start = abs_pos + 2;
        }

        // Search for // anywhere in the line (Samba/Unix UNC)
        let mut search_start = 0;
        while let Some(pos) = trimmed[search_start..].find("//") {
            let abs_pos = search_start + pos;
            let substr = &trimmed[abs_pos..];

            // Skip /// (empty host)
            if substr.starts_with("///") {
                search_start = abs_pos + 3;
                continue;
            }

            // Check for //host/share pattern
            let after_slash = &substr[2..];
            if let Some(slash_pos) = after_slash.find('/') {
                if slash_pos > 0 {
                    let host = &after_slash[..slash_pos];
                    let host_lower = host.to_lowercase();
                    if host_lower == "localhost"
                        || host_lower == "127.0.0.1"
                        || host_lower == "."
                        || host_lower == "?"
                    {
                        search_start = abs_pos + 2;
                        continue;
                    }
                    return true;
                }
            }
            search_start = abs_pos + 2;
        }
    }

    false
}

// ---------------------------------------------------------------------------
// Shell text classification
// ---------------------------------------------------------------------------

/// Classify a raw Bash command string as read-only or not.
///
/// This entry point is intentionally more conservative than the argv-level map:
/// Plan/Explore mode only auto-allows a single simple command. Shell syntax that
/// can hide extra execution or file writes is rejected before argv validation.
pub fn is_read_only_bash_command(command: &str) -> ReadOnlyResult {
    classify_shell_command_text(command, ShellDialect::Bash)
}

/// Classify a raw PowerShell command string as read-only or not.
///
/// This only permits simple external command invocations such as `git status`
/// or `rg pattern`. PowerShell pipelines, variables, script blocks, redirects,
/// and invocation operators remain approval-gated.
pub fn is_read_only_powershell_command(command: &str) -> ReadOnlyResult {
    classify_shell_command_text(command, ShellDialect::PowerShell)
}

fn classify_shell_command_text(command: &str, dialect: ShellDialect) -> ReadOnlyResult {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return ReadOnlyResult::ParseFailed("empty shell command".into());
    }

    if contains_vulnerable_unc_path(trimmed) {
        return ReadOnlyResult::NotReadOnly("command contains vulnerable UNC path".into());
    }

    if let Some(reason) = forbidden_shell_syntax(trimmed, dialect) {
        return ReadOnlyResult::NotReadOnly(reason);
    }

    let parsed = fallback::parse_shell_command(trimmed, ParseMode::FailClosedSecurity);
    if let Some(err) = parsed
        .diagnostics
        .iter()
        .find(|diag| diag.severity == DiagnosticSeverity::Error)
    {
        return ReadOnlyResult::ParseFailed(err.message.clone());
    }

    if parsed.segments.len() != 1 {
        return ReadOnlyResult::NotReadOnly("compound shell commands require approval".into());
    }

    let Some(segment) = parsed.segments.first() else {
        return ReadOnlyResult::ParseFailed("command did not parse into a segment".into());
    };

    if !segment.redirections.is_empty() {
        return ReadOnlyResult::NotReadOnly("shell redirections require approval".into());
    }
    if !segment.heredocs.is_empty() {
        return ReadOnlyResult::NotReadOnly("heredocs require approval".into());
    }

    let Some(simple) = &segment.command else {
        return ReadOnlyResult::ParseFailed("command did not parse into argv".into());
    };
    if simple.argv.is_empty() {
        return ReadOnlyResult::ParseFailed("empty argv".into());
    }

    is_read_only_shell_command(&simple.argv, trimmed)
}

fn forbidden_shell_syntax(command: &str, dialect: ShellDialect) -> Option<String> {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let chars: Vec<char> = command.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if escaped {
            escaped = false;
            i += 1;
            continue;
        }

        match dialect {
            ShellDialect::PowerShell if ch == '`' => {
                return Some("PowerShell escape/dynamic syntax requires approval".into());
            }
            _ if ch == '\\' && !in_single => {
                escaped = true;
                i += 1;
                continue;
            }
            _ => {}
        }

        if ch == '\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }

        if !in_single && !in_double {
            match ch {
                '\n' | '\r' | ';' | '|' | '&' | '<' | '>' => {
                    return Some("shell operators and redirections require approval".into());
                }
                '$' => {
                    return Some("shell expansions require approval".into());
                }
                '`' => {
                    return Some("shell command substitution requires approval".into());
                }
                '(' | ')' | '{' | '}' if dialect == ShellDialect::PowerShell => {
                    return Some(
                        "PowerShell expressions and script blocks require approval".into(),
                    );
                }
                _ => {}
            }
        }

        i += 1;
    }

    None
}

// ---------------------------------------------------------------------------
// Command name matching utilities
// ---------------------------------------------------------------------------

/// Try to match a command name + args against a known read-only command map.
///
/// Supports multi-level command keys like "git diff" or "gh pr view".
/// Returns the longest matching key and its config.
fn match_readonly_command<'a>(
    argv: &[String],
    map: &'a HashMap<String, ExternalCommandConfig>,
) -> Option<(&'a ExternalCommandConfig, Vec<String>)> {
    if argv.is_empty() {
        return None;
    }

    // Try longest match first (3-gram, 2-gram, 1-gram)
    let max_depth = argv.len().min(3);
    for depth in (1..=max_depth).rev() {
        let key = argv[0..depth].join(" ");
        if let Some(config) = map.get(&key) {
            let remaining: Vec<String> = argv[depth..].to_vec();
            return Some((config, remaining));
        }
    }

    // Try just the command basename
    let base = std::path::Path::new(&argv[0])
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(&argv[0]);

    if let Some(config) = map.get(base) {
        let remaining: Vec<String> = argv[1..].to_vec();
        return Some((config, remaining));
    }

    None
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Determine whether a shell command is read-only.
///
/// Returns `ReadOnlyResult::ReadOnly` if the command is safe to execute
/// without user approval (Plan mode, Explore mode, etc.).
///
/// The check uses known read-only command maps for git, gh, docker, rg,
/// pyright, and common external commands. Unknown commands are classified
/// as `Unsupported`.
pub fn is_read_only_shell_command(argv: &[String], raw_command: &str) -> ReadOnlyResult {
    if argv.is_empty() {
        return ReadOnlyResult::Unsupported("empty argv".into());
    }

    // Check UNC path vulnerability
    if contains_vulnerable_unc_path(raw_command) {
        return ReadOnlyResult::NotReadOnly("command contains vulnerable UNC path".into());
    }

    let command_base = std::path::Path::new(&argv[0])
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(&argv[0]);

    let _remaining: Vec<String> = if argv.len() > 1 {
        argv[1..].to_vec()
    } else {
        vec![]
    };

    // Determine command name for flag validation
    let command_name = match command_base {
        "git" | "gh" | "docker" => Some(command_base),
        _ => None,
    };

    // Try matching against known maps

    // 1. GIT
    if command_base == "git" || command_base == "git.exe" {
        let git_map = make_git_read_only_commands();
        if let Some((config, args)) = match_readonly_command(argv, &git_map) {
            if validate_flags(&args, config, command_name, raw_command) {
                return ReadOnlyResult::ReadOnly;
            }
            return ReadOnlyResult::NotReadOnly(
                "git command or flags are not read-only safe".into(),
            );
        }
        // No matching git subcommand — treat as write
        return ReadOnlyResult::NotReadOnly(format!(
            "unsupported git subcommand: {}",
            argv.get(1).map(|s| s.as_str()).unwrap_or("(none)")
        ));
    }

    // 2. GH
    if command_base == "gh" || command_base == "gh.exe" {
        let gh_map = make_gh_read_only_commands();
        if let Some((config, args)) = match_readonly_command(argv, &gh_map) {
            if validate_flags(&args, config, command_name, raw_command) {
                return ReadOnlyResult::ReadOnly;
            }
            return ReadOnlyResult::NotReadOnly(
                "gh command or flags are not read-only safe".into(),
            );
        }
        return ReadOnlyResult::NotReadOnly(format!(
            "unsupported gh subcommand: {}",
            argv.get(1).map(|s| s.as_str()).unwrap_or("(none)")
        ));
    }

    // 3. DOCKER
    if command_base == "docker" || command_base == "docker.exe" {
        let docker_map = make_docker_read_only_commands();
        if let Some((config, args)) = match_readonly_command(argv, &docker_map) {
            if validate_flags(&args, config, command_name, raw_command) {
                return ReadOnlyResult::ReadOnly;
            }
            return ReadOnlyResult::NotReadOnly(
                "docker command or flags are not read-only safe".into(),
            );
        }
        return ReadOnlyResult::NotReadOnly(format!(
            "unsupported docker subcommand: {}",
            argv.get(1).map(|s| s.as_str()).unwrap_or("(none)")
        ));
    }

    // 4. RIPGREP
    if command_base == "rg" || command_base == "ripgrep" {
        let rg_map = make_rg_read_only_commands();
        if let Some((config, args)) = match_readonly_command(argv, &rg_map) {
            if validate_flags(&args, config, command_name, raw_command) {
                return ReadOnlyResult::ReadOnly;
            }
            return ReadOnlyResult::NotReadOnly("rg flags are not read-only safe".into());
        }
    }

    // 5. PYRIGHT
    if command_base == "pyright" || command_base == "pyright-langserver" {
        let pyright_map = make_pyright_read_only_commands();
        if let Some((config, args)) = match_readonly_command(argv, &pyright_map) {
            if validate_flags(&args, config, command_name, raw_command) {
                return ReadOnlyResult::ReadOnly;
            }
            return ReadOnlyResult::NotReadOnly("pyright flags are not read-only safe".into());
        }
    }

    // 6. known external read-only commands
    let ext_map = make_external_readonly_commands();
    if let Some((config, args)) = match_readonly_command(argv, &ext_map) {
        // For simple commands with no flag restrictions (empty safe_flags
        // and no additional check), any remaining args are fine — the
        // command itself is inherently read-only. Combined short flags
        // like `ls -la` are accepted without expanding each flag.
        if config.safe_flags.is_empty() && config.additional_check.is_none() {
            return ReadOnlyResult::ReadOnly;
        }
        // Check that the command is EXACTLY a known external command
        // (no subcommand that might be dangerous)
        if argv.len() == 1 {
            return ReadOnlyResult::ReadOnly;
        }
        // With args: check that remaining args are all flags or simple paths
        let remaining = &argv[1..];
        // Allow non-flag args for simple commands (they're file paths)
        // Block redirects and shell metacharacters (handled by the caller)
        if remaining.iter().all(|a| !a.starts_with('-') || a == "--") {
            return ReadOnlyResult::ReadOnly;
        }
        // If there are flag-like args, validate them
        if validate_flags(&args, config, None, raw_command) {
            return ReadOnlyResult::ReadOnly;
        }
    }

    ReadOnlyResult::Unsupported(format!("unknown or unsupported command: {}", command_base))
}

// ---------------------------------------------------------------------------
// Helper: merge multiple HashMap sources
// ---------------------------------------------------------------------------

fn merge_maps(sources: &[HashMap<String, FlagArgType>]) -> HashMap<String, FlagArgType> {
    let mut result = HashMap::new();
    for source in sources {
        result.extend(source.iter().map(|(k, v)| (k.clone(), *v)));
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Helpers ---

    fn ro(argv: &[&str], raw: &str) -> ReadOnlyResult {
        let a: Vec<String> = argv.iter().map(|s| s.to_string()).collect();
        is_read_only_shell_command(&a, raw)
    }

    fn is_ro(argv: &[&str], raw: &str) -> bool {
        ro(argv, raw) == ReadOnlyResult::ReadOnly
    }

    fn is_not_ro(argv: &[&str], raw: &str) -> bool {
        matches!(ro(argv, raw), ReadOnlyResult::NotReadOnly(_))
    }

    fn is_unsupported(argv: &[&str], raw: &str) -> bool {
        matches!(ro(argv, raw), ReadOnlyResult::Unsupported(_))
    }

    // --- GIT ---

    #[test]
    fn test_git_status_read_only() {
        assert!(is_ro(&["git", "status"], "git status"));
        assert!(is_ro(&["git", "status", "-s"], "git status -s"));
        assert!(is_ro(
            &["git", "status", "--porcelain"],
            "git status --porcelain"
        ));
    }

    #[test]
    fn test_git_diff_read_only() {
        assert!(is_ro(&["git", "diff"], "git diff"));
        assert!(is_ro(&["git", "diff", "--cached"], "git diff --cached"));
        assert!(is_ro(
            &["git", "diff", "HEAD~1", "HEAD"],
            "git diff HEAD~1 HEAD"
        ));
    }

    #[test]
    fn test_git_log_read_only() {
        assert!(is_ro(&["git", "log"], "git log"));
        assert!(is_ro(
            &["git", "log", "--oneline", "-n", "5"],
            "git log --oneline -n 5"
        ));
        assert!(is_ro(
            &["git", "log", "--all", "--graph"],
            "git log --all --graph"
        ));
    }

    #[test]
    fn test_git_push_not_read_only() {
        assert!(is_not_ro(&["git", "push"], "git push"));
        assert!(is_not_ro(
            &["git", "push", "origin", "main"],
            "git push origin main"
        ));
    }

    #[test]
    fn test_git_commit_not_read_only() {
        assert!(is_not_ro(
            &["git", "commit", "-m", "msg"],
            "git commit -m msg"
        ));
    }

    #[test]
    fn test_git_checkout_not_read_only() {
        assert!(is_not_ro(
            &["git", "checkout", "branch"],
            "git checkout branch"
        ));
    }

    #[test]
    fn test_git_reset_not_read_only() {
        assert!(is_not_ro(&["git", "reset"], "git reset"));
    }

    #[test]
    fn test_git_clean_not_read_only() {
        assert!(is_not_ro(&["git", "clean", "-fd"], "git clean -fd"));
    }

    #[test]
    fn test_git_reflog_read_only() {
        assert!(is_ro(&["git", "reflog"], "git reflog"));
        assert!(is_ro(&["git", "reflog", "show"], "git reflog show"));
    }

    #[test]
    fn test_git_reflog_expire_not_read_only() {
        assert!(is_not_ro(&["git", "reflog", "expire"], "git reflog expire"));
    }

    #[test]
    fn test_git_branch_list_read_only() {
        assert!(is_ro(&["git", "branch", "-a"], "git branch -a"));
    }

    #[test]
    fn test_git_branch_delete_not_read_only() {
        assert!(is_not_ro(
            &["git", "branch", "-d", "old-branch"],
            "git branch -d old-branch"
        ));
    }

    #[test]
    fn test_git_fetch_not_read_only() {
        assert!(is_not_ro(&["git", "fetch", "--all"], "git fetch --all"));
        assert!(is_not_ro(
            &["git", "fetch", "--dry-run"],
            "git fetch --dry-run"
        ));
    }

    #[test]
    fn test_git_ls_files_read_only() {
        assert!(is_ro(&["git", "ls-files"], "git ls-files"));
        assert!(is_ro(&["git", "ls-files", "-c"], "git ls-files -c"));
    }

    #[test]
    fn test_git_describe_read_only() {
        assert!(is_ro(&["git", "describe"], "git describe"));
    }

    #[test]
    fn test_git_blame_read_only() {
        assert!(is_ro(&["git", "blame", "file.txt"], "git blame file.txt"));
    }

    #[test]
    fn test_git_grep_read_only() {
        assert!(is_ro(&["git", "grep", "pattern"], "git grep pattern"));
        assert!(is_ro(
            &["git", "grep", "-n", "pattern", "--", "*.rs"],
            "git grep -n pattern -- *.rs"
        ));
    }

    #[test]
    fn test_git_config_read_only() {
        assert!(is_ro(&["git", "config", "--list"], "git config --list"));
        assert!(is_ro(
            &["git", "config", "user.name"],
            "git config user.name"
        ));
    }

    #[test]
    fn test_git_config_write_not_read_only() {
        assert!(is_not_ro(
            &["git", "config", "user.name", "new-name"],
            "git config user.name new-name"
        ));
    }

    #[test]
    fn test_git_unknown_subcommand_unsupported() {
        assert!(is_not_ro(&["git", "foo"], "git foo"));
        assert!(is_not_ro(&["git", "bisect", "start"], "git bisect start"));
    }

    #[test]
    fn test_git_tag_list_read_only() {
        assert!(is_ro(&["git", "tag", "-l"], "git tag -l"));
    }

    #[test]
    fn test_git_diff_late_output_flag_not_read_only() {
        assert!(is_not_ro(
            &["git", "diff", "HEAD", "--output=/tmp/pwn.patch"],
            "git diff HEAD --output=/tmp/pwn.patch"
        ));
    }

    #[test]
    fn test_git_archive_output_not_read_only() {
        assert!(is_not_ro(
            &["git", "archive", "--output", "/tmp/archive.tar", "HEAD"],
            "git archive --output /tmp/archive.tar HEAD"
        ));
    }

    // --- GH ---

    #[test]
    fn test_gh_pr_view_read_only() {
        assert!(is_ro(&["gh", "pr", "view", "123"], "gh pr view 123"));
    }

    #[test]
    fn test_gh_pr_list_read_only() {
        assert!(is_ro(&["gh", "pr", "list"], "gh pr list"));
    }

    #[test]
    fn test_gh_issue_list_read_only() {
        assert!(is_ro(&["gh", "issue", "list"], "gh issue list"));
    }

    #[test]
    fn test_gh_run_list_read_only() {
        assert!(is_ro(&["gh", "run", "list"], "gh run list"));
    }

    #[test]
    fn test_gh_pr_merge_not_read_only() {
        assert!(is_not_ro(&["gh", "pr", "merge", "123"], "gh pr merge 123"));
    }

    #[test]
    fn test_gh_api_get_read_only() {
        assert!(is_ro(
            &["gh", "api", "/repos/owner/repo"],
            "gh api /repos/owner/repo"
        ));
    }

    #[test]
    fn test_gh_api_post_not_read_only() {
        assert!(is_not_ro(
            &["gh", "api", "POST", "/repos"],
            "gh api POST /repos"
        ));
    }

    // --- DOCKER ---

    #[test]
    fn test_docker_ps_read_only() {
        assert!(is_ro(&["docker", "ps"], "docker ps"));
        assert!(is_ro(&["docker", "ps", "-a"], "docker ps -a"));
    }

    #[test]
    fn test_docker_images_read_only() {
        assert!(is_ro(&["docker", "images"], "docker images"));
    }

    #[test]
    fn test_docker_inspect_read_only() {
        assert!(is_ro(
            &["docker", "inspect", "container"],
            "docker inspect container"
        ));
    }

    #[test]
    fn test_docker_run_not_read_only() {
        assert!(is_not_ro(&["docker", "run", "ubuntu"], "docker run ubuntu"));
    }

    // --- RIPGREP ---

    #[test]
    fn test_rg_read_only() {
        assert!(is_ro(&["rg", "pattern"], "rg pattern"));
        assert!(is_ro(
            &["rg", "-n", "pattern", "src/"],
            "rg -n pattern src/"
        ));
        assert!(is_ro(&["rg", "--json", "pattern"], "rg --json pattern"));
    }

    #[test]
    fn test_pyright_write_flags_not_read_only() {
        assert!(is_not_ro(
            &["pyright", "--createstub", "os"],
            "pyright --createstub os"
        ));
        assert!(is_not_ro(&["pyright", "--watch"], "pyright --watch"));
    }

    // --- EXTERNAL ---

    #[test]
    fn test_ls_read_only() {
        assert!(is_ro(&["ls"], "ls"));
        assert!(is_ro(&["ls", "-la"], "ls -la"));
    }

    #[test]
    fn test_cat_read_only() {
        assert!(is_ro(&["cat", "file.txt"], "cat file.txt"));
    }

    #[test]
    fn test_echo_read_only() {
        assert!(is_ro(&["echo", "hello"], "echo hello"));
    }

    #[test]
    fn test_unknown_command_unsupported() {
        assert!(is_unsupported(
            &["unknown_cmd", "--flag"],
            "unknown_cmd --flag"
        ));
    }

    // --- raw shell command text ---

    #[test]
    fn test_bash_text_read_only_safe_simple_command() {
        assert_eq!(
            is_read_only_bash_command("git status --short"),
            ReadOnlyResult::ReadOnly
        );
        assert_eq!(
            is_read_only_bash_command("rg pattern src"),
            ReadOnlyResult::ReadOnly
        );
    }

    #[test]
    fn test_bash_text_blocks_shell_syntax() {
        assert!(matches!(
            is_read_only_bash_command("git status > out.txt"),
            ReadOnlyResult::NotReadOnly(_)
        ));
        assert!(matches!(
            is_read_only_bash_command("git status && git diff"),
            ReadOnlyResult::NotReadOnly(_)
        ));
        assert!(matches!(
            is_read_only_bash_command("echo $(git status)"),
            ReadOnlyResult::NotReadOnly(_)
        ));
    }

    #[test]
    fn test_powershell_text_read_only_is_conservative() {
        assert_eq!(
            is_read_only_powershell_command("git status"),
            ReadOnlyResult::ReadOnly
        );
        assert!(matches!(
            is_read_only_powershell_command("git status | Select-Object -First 1"),
            ReadOnlyResult::NotReadOnly(_)
        ));
    }

    // --- UNC path ---

    #[test]
    fn test_vulnerable_unc_path_detected() {
        assert!(contains_vulnerable_unc_path(
            r"copy \\attacker\share\file.txt"
        ));
    }

    #[test]
    fn test_extended_length_unc_not_vulnerable() {
        assert!(!contains_vulnerable_unc_path(r"\\?\C:\path\to\file.txt"));
    }

    #[test]
    fn test_no_unc_path_not_vulnerable() {
        assert!(!contains_vulnerable_unc_path(r"cat /etc/passwd"));
    }

    // --- verify that Plan mode can use this ---
    #[test]
    fn test_read_only_commands_safe_for_plan_mode() {
        // These should all be ReadOnly
        let safe_commands = vec![
            vec!["git", "status"],
            vec!["git", "diff"],
            vec!["git", "log", "--oneline"],
            vec!["git", "show", "HEAD"],
            vec!["git", "describe"],
            vec!["git", "branch", "-a"],
            vec!["gh", "pr", "view", "123"],
            vec!["rg", "pattern"],
            vec!["ls", "-la"],
            vec!["cat", "file"],
            vec!["docker", "ps"],
        ];
        for argv in safe_commands {
            let raw = argv.join(" ");
            assert!(
                is_ro(&argv.iter().map(|s| *s).collect::<Vec<&str>>(), &raw),
                "expected ReadOnly for: {}",
                raw
            );
        }
    }

    #[test]
    fn test_write_commands_blocked_in_plan_mode() {
        // These should NOT be ReadOnly
        let dangerous_commands = vec![
            vec!["git", "clean", "-fd"],
            vec!["git", "checkout", "branch"],
            vec!["git", "push"],
            vec!["git", "commit", "-m", "msg"],
            vec!["git", "reset", "--hard"],
            vec!["git", "branch", "-d", "old"],
            vec!["git", "fetch", "--all"],
            vec!["gh", "pr", "merge", "123"],
            vec!["docker", "run", "ubuntu"],
        ];
        for argv in dangerous_commands {
            let raw = argv.join(" ");
            assert!(
                is_not_ro(&argv.iter().map(|s| *s).collect::<Vec<&str>>(), &raw)
                    || is_unsupported(&argv.iter().map(|s| *s).collect::<Vec<&str>>(), &raw),
                "expected NOT ReadOnly for: {}",
                raw
            );
        }
    }
}

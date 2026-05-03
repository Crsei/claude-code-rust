# UI Skeleton Surface Implementation Audit

Date: 2026-05-03
Scope: `crates/claude-code-rs/src/ui`

## Requirement Coverage

- Implement newly added Rust TUI skeleton files one file at a time.
  - Covered by concrete helpers/renderers under `agents`, `diff`, `hooks`, `lsp_recommendation`, `mcp`, `memory`, `messages`, `permissions`, `skills`, `tasks`, and `teams`.
  - `rg -n "skeleton|scaffold-only|Port rendering|future full-build|This file preserves" crates/claude-code-rs/src/ui` returns no matches.
- Use subagents where helpful.
  - Used focused subagents for diff, messages, mapping, and test strategy. Broad permission/rules attempts were abandoned after context failures; remaining permissions work was completed locally in smaller patches.
- Add snapshot tests for each feature.
  - Each implemented surface has deterministic snapshot coverage. Message and permission root snapshots call every helper in those surfaces at least once.
- Main-agent planning/checking.
  - Final checks were run locally after subagent integration, including formatting, typecheck, clippy, UI snapshots, e2e permissions, and release build.
- Create a human inspection document.
  - This file is the inspection artifact.
- Commit with Lore protocol.
  - Pending at time of this audit file; final commit message must include Lore trailers.

## Implemented Surface Groups

- `ui/agents/**`: agent definitions, validation, file paths, selectors, menus, editor, wizard steps, and snapshots.
- `ui/diff/**`: file list/detail/dialog rendering and snapshots.
- `ui/hooks/**`: hook config menu, event/matcher selection, prompt dialog, view mode, and snapshots.
- `ui/lsp_recommendation/**`: LSP recommendation menu and snapshot.
- `ui/mcp/**`: server/tool/capability views, reconnect, settings, elicitation, stdio/remote/agent menus, and snapshot.
- `ui/memory/**`: memory file selector/update notification and snapshot.
- `ui/messages/**`: assistant/system/user/team/tool-result helper surfaces and snapshots.
- `ui/permissions/**`: generic permission request models plus ask-user-question, shell, file, filesystem, sandbox, monitor, notebook, skill, web fetch, worker, hook, and rule-management surfaces with snapshot coverage.
- `ui/skills/**`: skills menu and snapshot.
- `ui/tasks/**`: background task, shell/remote/session/detail/workflow renderers and snapshot.
- `ui/teams/**`: team status/dialog renderers and snapshot.

## Verification Evidence

- `cargo fmt -p claude-code-rs`
- `cargo check -p claude-code-rs`
- `cargo clippy -p claude-code-rs --all-targets -- -D warnings`
- `cargo test -p claude-code-rs ui:: -- --nocapture --test-threads=1`
  - Result: 148 passed, 0 failed.
- `cargo test -p claude-code-rs --test e2e_permissions -- --nocapture`
  - Result: 2 passed, 0 failed.
- `cargo build --release`
- `git diff --check`
  - Result: exit 0; Git reported only CRLF normalization warnings for touched files.
- `Get-ChildItem -Recurse crates/claude-code-rs/src/ui -Filter *.snap.new`
  - Result: no temporary snapshot files.

## Snapshot Note

The command-palette snapshot `command_argument_help_all_commands_110w.snap` needed metadata refresh because the assertion line shifted. The parallel `ui::` test run can also produce padding-only drift in that shared snapshot; the final accepted verification uses `--test-threads=1`, where the suite is stable.

## Remaining Risks

- These are deterministic Rust TUI render/state helpers, not a full interactive port of every upstream React event path.
- Module names intentionally mirror upstream file names; narrow `clippy::module_inception` allows preserve that mapping for reviewability.
- Snapshot coverage locks text output and helper state, but does not replace manual end-to-end inspection of every terminal interaction path.

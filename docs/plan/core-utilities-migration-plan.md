# Core Utilities and Team/Swarm Migration Plan

Date: 2026-05-18

Scope:
- Source inventory: `claude-code-bun/src/utils/`
- Rust utility doc: `docs/utils/core-utilities.md`
- Team/Swarm doc: `docs/utils/teams-swarm.md`
- Ownership boundary: `docs/reference/CRATE_DEPENDENCY_TARGETS.md`

## Execution Objective

Close the `core-utilities.md` gap without turning `cc-utils` into a dumping
ground for every Bun `src/utils/` file. Use owner crates for domain behavior,
and reserve `cc-utils` for pure helper functions.

## Current Findings

| Area | Finding |
|---|---|
| `cc-utils` | Small but actively used by shell parsing, token accounting, git helpers, message preview helpers, cwd handling, and shell env construction |
| Duplicate utility state | `cc-utils/src/file_state_cache.rs` appears separate from the runtime `cc-tools::tool::FileStateCache` |
| Team/Swarm | InProcess path is already wired; major gaps are pane backends, backend detection, full permission sync, mailbox protocol completeness, and direct member messaging |
| Docs | `core-utilities.md` currently reads like a flat missing list; it needs owner-aware classification |

## First-Agent Recommended Tasks

1. Update `docs/utils/core-utilities.md` into an owner-aware matrix.
2. Decide and document whether `abort.rs` and `file_state_cache.rs` are active,
   staged, or redundant.
3. Add only low-risk pure helper modules to `cc-utils` if a real caller is ready.
4. Do not start tmux/iTerm2/Windows Terminal backend implementation until the
   Team mailbox and permission sync gaps are typed and stable.

## Team/Swarm Follow-Up Tasks

| Priority | Task | Owner files |
|---|---|---|
| High | Complete typed mailbox protocol | `crates/cc-teams/src/mailbox.rs`, `protocol.rs`, `send_message.rs`, `runner.rs` |
| High | Persist teammate self-session IDs and close/narrow `TEAMS-001` | `helpers.rs`, `reconnection.rs`, `types.rs` |
| High | Port permission/sandbox sync | new `permission_sync.rs`, `send_message.rs`, `runner.rs` |
| Medium | Add unified team discovery API | `cc-teams`, `cc-commands`, TUI dashboard/status call sites |
| Medium | Wire direct member input | TUI input modules and `cc-teams` mailbox send |
| Later | Pane backend registry and detection | `backend.rs`, new registry/detection modules |
| Later | Concrete tmux/iTerm2/Windows Terminal backends | new backend modules |

## Owner Rules

- `cc-utils`: pure helpers only.
- `cc-teams`: all team, teammate, mailbox, swarm, permission bridge, and pane backend behavior.
- `cc-permissions` / `cc-sandbox`: command risk, sandbox, and permission policy.
- `cc-session` / future `cc-state`: session restoration and runtime state.
- `cc-config`: paths, settings, features, user-agent, environment/config loading.
- `cc-mcp`, `cc-plugins`, `cc-browser`, `cc-computer-use`: keep their own Bun
  utility families in their owner crates.

## Validation Guidance

Use targeted checks only when implementation starts:

```bash
export CARGO_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/cargo
export RUSTUP_HOME=/data2-HDD-SATA-20T/Digital_avatar/haoweiyao/.rust/rustup
export PATH="$CARGO_HOME/bin:$PATH"

cargo test -p cc-utils
cargo test -p cc-teams
cargo check -p claude-code-rs
```

Do not run broad workspace validation until the batch is complete.

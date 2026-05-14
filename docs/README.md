# cc-rust docs map

This directory separates active status documents from historical records.

## Active entry points

- [WORK_STATUS.md](WORK_STATUS.md): current project status and active work areas.
- [FINAL_RELEASE_PLAN.md](FINAL_RELEASE_PLAN.md): final release sequencing, gates, remaining gaps, and expected release effects.
- [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md): remaining Full Build TODOs, runtime caveats, and intentional crops.
- [KNOWN_ISSUES.md](KNOWN_ISSUES.md): single active issue and review-finding tracker.
- [COMMAND_REFERENCE.md](COMMAND_REFERENCE.md), [CLI_REFERENCE.md](CLI_REFERENCE.md), [USAGE_GUIDE.md](USAGE_GUIDE.md): command and user-facing usage references.
- [reference/CRATE_MIGRATION_GUIDE.md](reference/CRATE_MIGRATION_GUIDE.md): operational checklist for moving `claude-code-rs` modules into workspace crates.
- [reference/CRATE_MIGRATION_TARGET_STATE.md](reference/CRATE_MIGRATION_TARGET_STATE.md): ideal acceptance state for a completed crate migration.
- [plan/crate-migration-phase-plan-2026-05-14.md](plan/crate-migration-phase-plan-2026-05-14.md): ordered phase plan for completing crate migration.
- [STORAGE.md](STORAGE.md): cc-rust data-root and path isolation reference.
- [DAEMON_OPERATIONS.md](DAEMON_OPERATIONS.md): daemon operation guide.
- [cloud-providers.md](cloud-providers.md): provider configuration reference.
- [RATATUI_UI_PARITY.md](RATATUI_UI_PARITY.md): Rust TUI parity matrix and residual UI work.

## Active plans

These are still planning or implementation references, not completed history:

- [computer-use-implementation-checklist.md](computer-use-implementation-checklist.md)
- [session-export-implementation-guide.md](session-export-implementation-guide.md)
- [ipc-refactor-plan.md](ipc-refactor-plan.md)
- [traceable-logging-plan.md](traceable-logging-plan.md)
- [daemon-usability-plan.md](daemon-usability-plan.md)
- [ui-parity-update-plan.md](ui-parity-update-plan.md)
- [superpowers/plans/2026-04-09-pty-commands-and-multi-turn.md](superpowers/plans/2026-04-09-pty-commands-and-multi-turn.md)
- [superpowers/plans/2026-04-11-team-memory-sync.md](superpowers/plans/2026-04-11-team-memory-sync.md)
- [superpowers/plans/2026-04-12-tools-commands-test-coverage.md](superpowers/plans/2026-04-12-tools-commands-test-coverage.md)
- [superpowers/specs/2026-04-11-team-memory-sync-design.md](superpowers/specs/2026-04-11-team-memory-sync-design.md)
- [superpowers/specs/2026-04-20-workspace-split-design.md](superpowers/specs/2026-04-20-workspace-split-design.md)

## Archive policy

- Completed implementation notes, closed phase records, historical issue reports, and old review documents live under [archive/](archive/).
- Long raw review reports live under [archive/issues/](archive/issues/); active summaries live in [KNOWN_ISSUES.md](KNOWN_ISSUES.md).
- When a TODO is implemented, move its detailed completion note to archive and leave only a short current-state reference in active docs.
- Do not keep completed phase logs at the top of [WORK_STATUS.md](WORK_STATUS.md) or [IMPLEMENTATION_GAPS.md](IMPLEMENTATION_GAPS.md).

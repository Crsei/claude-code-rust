# Workspace Crate Extraction Execution Plan

Date: 2026-05-10
Parent plan: `.omx/plans/claude-code-rs-to-crates-refactor-plan-2026-05-10.md`
Purpose: execution control plan for the six module-level subplans.
Mode: planning and runner setup; no Rust source implementation in this commit.

## Execution Defaults

- Model: `gpt-5.5`
- Reasoning effort: `medium`
- Visible output: concise, no long reasoning transcripts.
- Batch policy: dynamic batch sizing, start small, grow after clean batches,
  shrink on failures.
- Commit policy: commit only green batches; do not stage paths that were dirty
  before the runner started.
- Review policy: `[checkpoint]`, `[review]`, and `[final]` tasks always run as
  single-task batches.
- Error policy: use `PASS`, `WARNING`, `BLOCKER`, and `ERROR` in task output.

## Subplans

| Lane | Plan |
| --- | --- |
| tools/tasks | `.omx/plans/tools-tasks-crate-extraction-execution-subplan-2026-05-10.md` |
| api/models | `.omx/plans/api-models-extraction-execution-subplan-2026-05-10.md` |
| engine/query | `.omx/plans/engine-query-core-extraction-execution-subplan-2026-05-10.md` |
| ipc/protocol/client/runtime | `.omx/plans/ipc-protocol-runtime-extraction-execution-subplan-2026-05-10.md` |
| commands | `.omx/plans/commands-crate-extraction-execution-subplan-2026-05-10.md` |
| ui | `.omx/plans/cc-ui-extraction-execution-subplan-2026-05-10.md` |

## Lane Order

1. `api/models`: remove root API/model coupling before core runtime movement.
2. `engine/query`: establish runtime ownership.
3. `tools/tasks`: move task/tool ownership after core seams are explicit.
4. `ipc/protocol/client/runtime`: split wire/client/runtime after core seams.
5. `commands`: move user-facing command implementation once dependency crates
   are real.
6. `ui`: late extraction only after IPC/client boundaries are stable.

This order is intentionally conservative. If a lane hits a blocker, the runner
stops by default and records the blocker before later lanes execute.

## Dynamic Task Allocation

Use `scripts/run-workspace-crate-extraction-omx.ps1`.

Recommended dry run:

```powershell
.\scripts\run-workspace-crate-extraction-omx.ps1 -DryRun
```

Recommended execution:

```powershell
.\scripts\run-workspace-crate-extraction-omx.ps1
```

The wrapper:

- reads one task per line from
  `docs/scripts/workspace-crate-extraction-omx-tasks-2026-05-10.txt`;
- sends each batch to `scripts/codex-task-sequence.ps1`;
- fixes model/reasoning to `gpt-5.5` + `medium`;
- gives each task the global execution contract;
- keeps review/checkpoint/final tasks as single-task batches;
- grows batch size after clean batches and shrinks after failures;
- writes batch summaries under
  `target/codex-runs/workspace-crate-extraction-omx`;
- commits only new paths changed after the wrapper starts.

## Review Plan

Review gates are embedded in the task file. A review task must verify:

- targeted tests and `cargo fmt --all --check` status for the lane;
- dependency graph changes from `cargo metadata --no-deps`;
- file-size/refactor guard findings;
- behavior drift risks;
- explicit error visibility and avoidance of redundant defensive wrappers;
- effects, defects, and follow-ups are recorded.

The next build task must not run after a review reports `BLOCKER` or `ERROR`
unless the runner is explicitly invoked with `-ContinueOnError` for diagnostic
collection.

## Refactor Detection

The runner blocks commit when:

- a changed Rust source file exceeds the maximum line threshold;
- a rename/copy storm indicates the move is too broad for review;
- a task exits non-zero;
- a guard emits `BLOCKER` or `ERROR`.

The runner warns when:

- a changed Rust source file exceeds the warning threshold;
- one batch changes too many files;
- one file has excessive diff churn.

Default thresholds are intentionally adjustable on the wrapper command line.

## Reporting

Each batch writes JSON and Markdown summaries to the output directory. The final
execution tasks must also update:

`docs/archive/workspace-crate-extraction-execution-tracker-2026-05-10.md`

Required sections:

- implemented effects;
- defects and known divergences;
- follow-up tracking with owners;
- line-size/refactor findings;
- tests and verification evidence;
- dependency graph notes;
- explicit errors introduced or clarified.

## Risks

- Current worktree is dirty. The wrapper snapshots the baseline dirty path set
  and avoids committing those paths.
- `.omx` plan files may be ignored by default. This planning commit must force
  add the plan files intentionally.
- Full workspace tests may be expensive; lane tasks should run targeted gates
  before final workspace gates.
- UI extraction should not begin until IPC protocol/client boundaries are
  stable.

# Scripts

This directory documents runnable helper scripts under `scripts/`.

## `codex-task-sequence.ps1`

Run a sequence of Codex tasks in the same terminal window. Each task is executed as a fresh `codex exec` session, so the agent starts from a new session boundary while the shell window stays open.

### Inline tasks

```powershell
.\scripts\codex-task-sequence.ps1 -Tasks @(
  "任务1：..."
  "任务2：..."
  "任务3：..."
  "任务4：..."
  "任务5：..."
  "任务6：..."
  "任务7：..."
  "任务8：..."
  "任务9：..."
) -Model "gpt-5.5" -ReasoningEffort "medium"
```

### File-driven tasks

Create `codex-tasks.txt` with one task per line:

```text
任务1：...
任务2：...
任务3：...
任务4：...
任务5：...
任务6：...
任务7：...
任务8：...
任务9：...
```

Then run:

```powershell
.\scripts\codex-task-sequence.ps1 -TasksFile .\codex-tasks.txt -Model "gpt-5.5" -ReasoningEffort "medium"
```

### Useful options

- `-WorkDir`: working directory for each `codex exec` run
- `-OutputDir`: where per-task last-message files are written
- `-Model`: pass through `codex exec --model`, for example `gpt-5.5`
- `-ReasoningEffort`: pass through `codex exec --config model_reasoning_effort="..."`; valid values are `minimal`, `low`, `medium`, `high`, and `xhigh`
- `-ContinueOnError`: keep going after a failed task
- `-Profile` and `-Sandbox`: pass through Codex CLI settings

## `run-ratatui-ui-parity-omx.ps1`

Run the ratatui UI parity execution lane through `omx exec`, using
`codex-task-sequence.ps1` as the per-task transport.

The wrapper fixes model and reasoning to `gpt-5.5` + `medium`, batches tasks
dynamically, writes batch summaries under `target/codex-runs/ratatui-ui-parity-omx`,
runs diff/refactor guards, and commits green batches without staging paths that
were already dirty when the wrapper started.

Dry run:

```powershell
.\scripts\run-ratatui-ui-parity-omx.ps1 -DryRun
```

Execute:

```powershell
.\scripts\run-ratatui-ui-parity-omx.ps1
```

Useful options:

- `-TasksFile`: defaults to `docs/scripts/ratatui-ui-parity-omx-tasks.txt`
- `-SkipCommit`: run tasks and guards without automatic commits
- `-ContinueOnError`: continue after a failed batch to collect more diagnostics
- `-MaxBatchSize`, `-MinBatchSize`, `-InitialBatchSize`: tune dynamic batch sizing

## `run-workspace-crate-extraction-omx.ps1`

Run the workspace crate-extraction execution plan through
`codex-task-sequence.ps1` with fixed `gpt-5.5` + `medium`, dynamic batching,
review checkpoints, commit-on-green, Rust file-size/refactor guards, final
validation, and a final run report.

Dry run:

```powershell
.\scripts\run-workspace-crate-extraction-omx.ps1 -DryRun
```

Execute:

```powershell
.\scripts\run-workspace-crate-extraction-omx.ps1
```

Useful options:

- `-TasksFile`: defaults to `docs/scripts/workspace-crate-extraction-omx-tasks-2026-05-10.txt`
- `-SkipCommit`: run tasks and guards without automatic commits
- `-CommitBaselineDirtyChanges`: defaults to `$true`; baseline-dirty files are committed if their content changes during a batch
- `-ContinueOnError`: continue after a failed batch to collect diagnostics
- `-SkipFinalValidation`: skip the final `cargo fmt/check/clippy/test` validation sequence
- `-InitialBatchSize`, `-MinBatchSize`, `-MaxBatchSize`: tune dynamic task allocation
- `-WarnRustFileLines` and `-MaxRustFileLines`: tune refactor/file-size detection

Outputs:

- `batch-XX.summary.md/json`: per-batch tasks, changed files, guard findings, diagnostics, and last-message paths
- `failures.md/jsonl`: failed batch reasons, including test/script failures and agent-reported `ERROR` / `BLOCKER`
- `final-validation/`: logs for `cargo fmt --all --check`, `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`
- `final-report.md/json`: run-level summary, stopped-early reason, validation results, and git status sample

Default failure behavior: stop after the first failed batch, record the failure
reason, run final validation, and write the final report. Use
`-ContinueOnError` only when you intentionally want later tasks to run on the
post-failure worktree for diagnostic collection.

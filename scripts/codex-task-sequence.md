# codex-task-sequence.ps1

Run a sequence of Codex tasks in the same terminal window. Each task starts a fresh `codex exec` session, while the PowerShell process keeps running through the task list in order.

## Recommended command

For the P1 fail-fast follow-up tasks, run from the repository root:

```powershell
.\scripts\codex-task-sequence.ps1 `
  -TasksFile .\docs\scripts\p1-defensive-fail-fast-after-phase1-tasks.txt `
  -WorkDir . `
  -OutputDir .\target\codex-runs\p1-defensive-after-phase1 `
  -Sandbox danger-full-access `
  -Model "gpt-5.5" `
  -ReasoningEffort "medium"
```

By default, the script stops on the first failed task. Add `-ContinueOnError` only when you want later tasks to run even after a failure.

## Task file format

Use one task per line. Empty lines and lines starting with `#` are ignored.

```text
# comment
Task 1 - ...
Task 2 - ...
Task 3 - ...
```

## Inline task format

```powershell
.\scripts\codex-task-sequence.ps1 -Tasks @(
  "Task 1 - ..."
  "Task 2 - ..."
  "Task 3 - ..."
) -Model "gpt-5.5" -ReasoningEffort "medium"
```

## Useful options

- `-TasksFile`: path to the one-task-per-line task file.
- `-Tasks`: inline task array; used instead of `-TasksFile` when provided.
- `-WorkDir`: working directory passed to each `codex exec --cd`.
- `-OutputDir`: directory for `task-XX.last-message.txt` files.
- `-Model`: passed through as `codex exec --model`.
- `-ReasoningEffort`: passed through as `--config model_reasoning_effort="..."`; valid values are `minimal`, `low`, `medium`, `high`, and `xhigh`.
- `-Sandbox`: passed through as `codex exec --sandbox`; default is `danger-full-access`.
- `-Profile`: passed through as `codex exec --profile`.
- `-ContinueOnError`: keep running subsequent tasks after a non-zero exit.

## Output

Each task writes its final assistant message to:

```text
<OutputDir>\task-XX.last-message.txt
```

Use these files when preparing follow-up documentation or checking which task failed.

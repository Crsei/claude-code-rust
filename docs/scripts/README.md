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

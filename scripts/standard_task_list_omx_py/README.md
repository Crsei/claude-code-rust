# standard_task_list_omx_py 说明文档

`scripts/standard_task_list_omx_py/` 是标准任务列表批处理计划的 Python 实现包。它不是普通业务代码，也不是 Rust 功能模块；它是一个本地编排工具，用来把大规模任务列表拆成可审查的 Codex 批次，执行 guard，写出可恢复的运行产物，并在通过时提交 batch。

常规用户入口仍然是：

```powershell
.\scripts\run-standard-task-list-omx.ps1
```

调试 Python 实现时可以直接使用：

```powershell
python .\scripts\standard_task_list_omx.py --dry-run
python .\scripts\standard_task_list_omx_supervisor.py --plan-only
```

## 设计目标

- 将完整 standard task list 任务清单拆成小 batch，降低一次性改动风险。
- 固定 Codex 执行参数，避免不同 batch 使用不同模型或 reasoning 配置。
- 对每个 batch 记录可读、可机器解析的 summary。
- 用 git diff、Rust 文件行数、单文件 diff 大小、agent last-message 等信号阻止不可审查的改动。
- 在 green batch 后自动提交，保持历史可回滚。
- 在失败或中断后通过 supervisor 识别已完成任务并续跑剩余任务。

## 入口关系

```text
scripts/run-standard-task-list-omx.ps1
    -> scripts/standard_task_list_omx.py
        -> standard_task_list_omx_py.cli.main()

scripts/standard_task_list_omx_supervisor.py
    -> standard_task_list_omx_py.supervisor.main()
```

`run-standard-task-list-omx.ps1` 是兼容 PowerShell 入口。它负责查找 `python` 或 `py -3`，再把参数转成 Python CLI 参数。真正的 runner 逻辑在 `cli.py` 中。

`standard_task_list_omx_supervisor.py` 是续跑控制入口。它不直接执行任务细节，而是读取已有 batch summary，生成 remaining tasks 文件，再调用 Python runner。

## 目录内容

| 文件 | 主要职责 |
| --- | --- |
| `__init__.py` | 包说明。 |
| `cli.py` | runner CLI、动态 batch 主循环、调用底层 `codex-task-sequence.ps1`、green/failure batch 处理。 |
| `tasks.py` | 读取任务文件，识别 checkpoint/review/final 任务，计算下一个 batch 大小。 |
| `common.py` | 通用常量、路径解析、git 命令封装、文本写入、统一时间格式。 |
| `git_state.py` | 读取 worktree 改动、记录 baseline dirty 签名、计算当前 batch 应纳入的 changed paths。 |
| `guards.py` | diff guard、Rust 文件行数 guard、rename/copy guard、agent last-message finding 解析。 |
| `blocker_review.py` | 只读 blocker-risk review agent 调用、review 结论解析、风险 finding 改写。 |
| `artifacts.py` | 写 `batch-XX.summary.md/json`、`failures.md/jsonl`，并计算 batch 状态。 |
| `commit.py` | 对 green batch 执行 `git add`、生成 Lore 风格 commit message、执行 `git commit`。 |
| `validation.py` | 最终 `cargo fmt/check/clippy/test` 验证、日志写入、run-level final report。 |
| `supervisor.py` | 续跑、历史 summary 扫描、remaining task 生成、runner 故障分类、可选 repair 会话。 |

配套测试：

```text
scripts/tests/test_standard_task_list_omx.py
```

## Runner 工作流

Runner 入口是 `standard_task_list_omx_py.cli.main()`，核心流程如下：

1. 解析 CLI 参数。
2. 定位 repo root、任务文件、底层 runner、工作目录和输出目录。
3. 读取完整任务清单，忽略空行和 `#` 注释。
4. 记录启动时已经 dirty 的路径及其内容签名。
5. 按动态 batch size 从任务清单中取任务。
6. 为当前 batch 写入 `batch-XX.tasks.txt`，每行加上 `GLOBAL_CONTRACT`。
7. 调用 `scripts/codex-task-sequence.ps1` 执行本 batch。
8. 读取当前 git changed paths，并和启动时 baseline dirty 签名对比，得到本 batch changed paths。
9. 执行 diff/file-size/agent-report guards。
10. 底层 `codex-task-sequence.ps1` 在每个 task 结束后写 `task-XX.summary.json`，并刷新当前 batch 的 `task-report.md/json`。
11. 如果出现 `BLOCKER-RISK`，启动只读 blocker-risk review agent，写入风险结论。
12. 写 batch summary 和 failure log。
13. 每个 batch 结束或 runner 内部中断时刷新 run-level `final-report.md/json` 和 `execution-report.md/json`。
14. 如果 batch green，按配置自动提交并尝试放大 batch size。
15. 如果 batch failed，缩小 batch size，并按配置停止或继续。
16. 全部 batch 结束后，除非跳过，执行最终 cargo 验证。
17. 再次写 `final-report.md/json`，并用是否存在失败决定进程退出码。

## Runner 参数

直接 Python 入口：

```powershell
python .\scripts\standard_task_list_omx.py [options]
```

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--tasks-file` | `codex-tasks.txt` | 完整任务清单。 |
| `--runner` | `scripts/codex-task-sequence.ps1` | 被调用的底层任务序列器。 |
| `--codex` | `codex` | 传给 PowerShell runner 的 Codex 可执行文件名。 |
| `--work-dir` | `.` | 底层 Codex 执行工作目录。 |
| `--output-root` | `target/codex-runs/standard-task-list-omx` | batch summary、failure、validation、final report 输出目录。 |
| `--model` | `gpt-5.5` | 固定选择项，目前只允许 `gpt-5.5`。 |
| `--reasoning-effort` | `medium` | 固定选择项，目前只允许 `medium`。 |
| `--sandbox` | `danger-full-access` | 传给底层 Codex 的 sandbox。 |
| `--initial-batch-size` | `1` | 初始 batch 大小。 |
| `--min-batch-size` | `1` | batch size 下限。 |
| `--max-batch-size` | `3` | batch size 上限。 |
| `--warn-rust-file-lines` | `500` | Rust 文件 warning 行数阈值。 |
| `--max-rust-file-lines` | `2000` | Rust 文件 blocker-risk 行数阈值。 |
| `--max-files-per-batch` | `12` | 单 batch 改动文件数 warning 阈值。 |
| `--max-per-file-diff-lines` | `600` | 单文件 diff 行数 warning 阈值。 |
| `--max-oversized-rust-files-before-blocker` | `3` | 单 batch 中超过 `--max-rust-file-lines` 的 Rust 文件达到该数量时标记 `BLOCKER-RISK`。 |
| `--skip-blocker-review` | 关闭 | 不启动只读 blocker-risk review agent。 |
| `--blocker-review-sandbox` | `read-only` | blocker-risk review agent 使用的 sandbox。 |
| `--blocker-review-timeout-seconds` | `900` | blocker-risk review agent 超时时间。 |
| `--commit-baseline-dirty-changes` | 开启 | baseline dirty 文件如果在 batch 中发生内容变化，也纳入 changed paths。 |
| `--no-commit-baseline-dirty-changes` | 关闭 | baseline dirty 文件即使变化也不纳入 changed paths。 |
| `--continue-on-error` | 关闭 | batch 失败后继续后续 batch。 |
| `--skip-commit` | 关闭 | green batch 不自动 commit。 |
| `--skip-final-validation` | 关闭 | 不运行最终 cargo 验证。 |
| `--dry-run` | 关闭 | 只打印 batch 计划，不启动底层 Codex。 |

PowerShell wrapper 参数与这些 Python 参数基本一一对应，命名改为 PowerShell 风格，例如 `-SkipCommit` 对应 `--skip-commit`。

## 任务文件规则

任务文件由 `tasks.load_task_list()` 读取：

- 使用 `utf-8-sig`，兼容带 BOM 文件。
- 空行会被忽略。
- 以 `#` 开头的行会被忽略。
- 其他非空行作为一个任务。

`tasks.next_batch_size()` 会保护 checkpoint 类任务：

- `[checkpoint]` 开头的任务单独一个 batch。
- `[review]` 开头的任务单独一个 batch。
- `[final]` 开头的任务单独一个 batch。
- 如果当前 batch 前方遇到 checkpoint/review/final，会提前截断，保证该任务从新 batch 开始。

## Batch 动态调整

`cli.RunState` 维护运行中状态：

- `batch_size`：当前 batch 大小。
- `clean_batch_count`：连续通过的 batch 数。
- `batch_number`：已启动的 batch 编号。
- `index`：下一条任务在完整任务清单中的索引。
- `completed_task_count`：已计入完成的任务数。
- `had_failure`、`stopped_early`、`stop_reason`：失败和提前停止状态。
- `validation_results`：最终验证结果。

调整规则：

- 初始 batch size 会被夹在 `min` 和 `max` 之间。
- green batch 后 `clean_batch_count += 1`。
- 连续两个 green batch 后，如果未达到 `max_batch_size`，`batch_size += 1`。
- failed batch 后 `batch_size -= 1`，但不低于 `min_batch_size`。
- 默认 failed batch 会停止；开启 `--continue-on-error` 后继续。

## GLOBAL_CONTRACT

`common.GLOBAL_CONTRACT` 会被加到每个下发给 Codex 的任务前面。它是 runner 和子任务 agent 的本地契约，要求子任务：

- 简洁输出，不写长 reasoning transcript。
- 使用 runner 固定的 `gpt-5.5` + `medium`。
- 只在独立、边界清晰的工作中使用 native subagents。
- 留在任务所有权范围内。
- 不执行 git commit，因为 commit 由 runner 统一负责。
- 明确返回 `PASS` / `WARNING` / `BLOCKER` / `ERROR` 信号。
- 在修改代码或文档时记录 effects、defects、follow-ups。
- Rust 文件超过 guard 阈值时优先拆分，而不是继续增大文件。

如果子任务 last-message 中出现这些信号，`guards.agent_reported_findings()` 会把它们纳入 batch finding。

## Git changed paths 策略

Runner 启动时会调用 `git_state.git_changed_paths()`，读取：

- `git diff --name-only HEAD --`
- `git ls-files --others --exclude-standard`

这些路径构成 baseline dirty paths。随后 `path_signature_map()` 记录每个 baseline dirty path 的签名：

- 优先使用 `git hash-object -- <path>` 得到 blob hash。
- 如果 hash-object 失败，退回文件大小和 mtime。
- 如果文件不存在，签名为 `missing`。

每个 batch 结束后，`batch_changed_paths()` 会重新读取当前 changed paths，并按策略筛选：

- 启动后新增 dirty 的路径总是属于当前 batch。
- 启动前已 dirty 的路径默认只有在内容签名变化后才属于当前 batch。
- 如果关闭 `--commit-baseline-dirty-changes`，启动前已 dirty 的路径不会被当前 batch 提交。

这可以避免 runner 误提交用户或其他任务早已存在的无关改动，同时允许 batch 对已有 dirty 文件做增量修改时被记录。

## Guard 规则

`guards.invoke_diff_guards()` 对当前 batch changed paths 做检查。

### 文件数

如果单 batch changed paths 数量超过 `--max-files-per-batch`，产生：

```text
WARNING: batch changed <n> files; consider reducing batch size for reviewability
```

### rename/copy

通过 `git diff --name-status HEAD -- <paths>` 统计 rename/copy 条目。超过 5 个时产生 blocker-risk 标记，但不会中断 runner：

```text
BLOCKER-RISK: batch has <n> rename/copy entries; split mechanical moves if review becomes unclear
```

### Rust 文件行数

只检查 `.rs` 文件。

- 超过 `warn` 阈值但未超过 `max`：`WARNING`。
- HEAD 中已经超过 `max`，本 batch 没有继续增大：`WARNING`。
- 超过 `max` 的 Rust 文件少于 `--max-oversized-rust-files-before-blocker`：`WARNING`。
- 超过 `max` 的 Rust 文件达到 `--max-oversized-rust-files-before-blocker`：`BLOCKER-RISK`。

`BLOCKER-RISK` 只表示“从这个 batch 的首个任务开始存在 blocker 限制风险”。它不会让 runner 中断，也不会阻止自动提交；summary 中会记录对应任务和 review 结论。

### Blocker-risk review

如果出现 `BLOCKER-RISK`，且未启用 `--skip-blocker-review`，runner 会启动一个只读 Codex review agent。这个 agent 不修改文件，只检查当前 diff 和风险 finding，并输出：

```text
BLOCKER_REVIEW: WAIVE
```

或：

```text
BLOCKER_REVIEW: KEEP
```

`WAIVE` 会把风险改写为 `WAIVED-BLOCKER-RISK`；`KEEP` 会继续保留 `BLOCKER-RISK`。两种结果都不会中断运行，都会让 batch 以 `WARNING` 状态继续。

### 单文件 diff 行数

通过 `git diff --numstat HEAD -- <paths>` 统计新增和删除行数。单文件 changed lines 超过 `--max-per-file-diff-lines` 时产生 `WARNING`。

### Agent last-message

`guards.agent_reported_findings()` 扫描 `batch-XX/task-*.last-message.txt`，识别：

- 行首 `ERROR: ...`
- 行首 `BLOCKER: ...`
- 行首 `WARNING: ...`
- `Status: ERROR ...`
- `Status: BLOCKER ...`

这些会成为 batch finding。子任务 agent 明确报告的 `ERROR` 和 `BLOCKER` 仍会阻止 batch 进入 commit；guard 限制类风险使用 `BLOCKER-RISK`，不会阻止继续运行。

## Batch 状态

状态由 `artifacts.batch_status()` 计算：

| 状态 | 条件 | 是否可提交 |
| --- | --- | --- |
| `PASS` | 底层 runner 退出码为 0，且没有 finding。 | 是 |
| `WARNING` | 底层 runner 退出码为 0，且只有 warning、`BLOCKER-RISK` 或 `WAIVED-BLOCKER-RISK` finding。 | 是 |
| `BLOCKER` | 底层 runner 退出码为 0，但存在 agent 报告的 blocker/error finding。 | 否 |
| `ERROR` | 底层 runner 退出码非 0。 | 否 |

`WARNING` 是可完成状态，因为它表示可审查但需要维护者注意。`BLOCKER-RISK` 属于 warning 级别，只做风险标记，不中断运行；`BLOCKER` 和 `ERROR` 是停止状态。

## 输出产物

默认输出根目录：

```text
target/codex-runs/standard-task-list-omx/
```

常见产物：

| 文件或目录 | 来源 | 内容 |
| --- | --- | --- |
| `batch-XX.tasks.txt` | `cli.run_batch()` | 当前 batch 下发给 Codex 的任务，每行含 `GLOBAL_CONTRACT`。 |
| `batch-XX/` | `codex-task-sequence.ps1` | 底层每个 task 的 last-message 等输出。 |
| `batch-XX/task-YY.summary.json` | `codex-task-sequence.ps1` | 单个 task 的状态、退出码、改动文件、last-message 路径、输出目录、中断原因。 |
| `batch-XX/task-report.md/json` | `codex-task-sequence.ps1` | 当前 batch 内已执行 task 的滚动报告；每个 task 结束后刷新。 |
| `batch-XX.summary.json` | `artifacts.write_batch_artifacts()` | batch、status、exit code、tasks、changed files、guard findings、diagnostics、last-message files。 |
| `batch-XX.summary.md` | `artifacts.write_batch_artifacts()` | 人可读 batch 摘要。 |
| `batch-XX.commit-message.txt` | `commit.commit_batch()` | 自动提交使用的 Lore commit message。 |
| `failures.jsonl` | `artifacts.write_failure_record()` | 每个失败 batch 一行 JSON。 |
| `failures.md` | `artifacts.write_failure_record()` | 人可读失败记录。 |
| `execution-report.md/json` | `validation.write_task_execution_report()` | run-level task 执行汇总，聚合每个 task 的 changed files、结果、中间输出和中断原因。 |
| `final-validation/` | `validation.invoke_final_validation()` | 最终 cargo 验证日志与 summary。 |
| `final-report.json` | `validation.write_run_report()` | run-level 机器可读报告。 |
| `final-report.md` | `validation.write_run_report()` | run-level 人可读报告。 |

`batch-XX.summary.json` 的核心字段：

```json
{
    "batch": 1,
    "status": "PASS",
    "exit_code": 0,
    "model": "gpt-5.5",
    "reasoning_effort": "medium",
    "tasks": [],
    "changed_files": [],
    "guard_violations": [],
    "diagnostics": [],
    "last_message_files": [],
    "written_at": "..."
}
```

Supervisor 依赖 `status` 和 `tasks` 字段判断哪些任务已经完成。

### Task execution report

每个 task 结束后，`codex-task-sequence.ps1` 会立即写：

```text
batch-XX/task-YY.summary.json
batch-XX/task-report.md
batch-XX/task-report.json
```

`task-YY.summary.json` 记录：

- task 编号和 task 文本。
- `PASS` / `ERROR` 状态和 exit code。
- 这个 task 前后 git changed paths 签名差异推导出的 changed files。
- `task-YY.last-message.txt` 路径。
- 当前 batch 输出目录。
- started/ended 时间。
- 如果任务失败或命令异常，记录 `interruption_reason`。

每个 batch 结束、runner 中断、最终验证结束时，Python runner 会刷新：

```text
execution-report.md
execution-report.json
```

这份 run-level report 聚合所有 `batch-*/task-*.summary.json`，用于从一个位置查看每个任务修改了哪些文件、执行结果、完成情况、中间输出位置和中断原因。`final-report.md` 会链接到这两份 execution report。

## 自动提交

`commit.commit_batch()` 只处理当前 batch changed paths：

1. 没有 changed paths 时直接返回。
2. 执行 `git add -- <changed_paths>`。
3. 如果 staged diff 为空，直接返回。
4. 生成 `batch-XX.commit-message.txt`。
5. 执行 `git commit -F <message_path>`。

提交信息遵守本仓库 Lore Commit Protocol，包含 intent line、约束、被拒绝的替代方案、confidence、scope-risk、directive、tested/not-tested。

如果 commit 失败，`cli.handle_green_batch()` 会把 commit 失败写成 `ERROR` finding，重写 batch summary，写 failure record，并按 `--continue-on-error` 决定是否停止。

## 最终验证

`validation.invoke_final_validation()` 会按固定顺序执行：

```powershell
cargo fmt --all --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

每个命令的 stdout/stderr 会同时输出到终端和日志文件：

```text
target/codex-runs/standard-task-list-omx/final-validation/fmt.log
target/codex-runs/standard-task-list-omx/final-validation/check.log
target/codex-runs/standard-task-list-omx/final-validation/clippy.log
target/codex-runs/standard-task-list-omx/final-validation/test.log
```

任一命令非零退出时，runner 会标记整体失败，并在 `final-report.md/json` 中记录。

## Supervisor 工作流

Supervisor 入口是 `standard_task_list_omx_py.supervisor.main()`。它用于恢复中断或失败后的长任务队列。

核心流程：

1. 解析 supervisor 参数和 `--` 后的 runner 透传参数。
2. 读取完整任务清单。
3. 扫描观察目录中的 `batch-*.summary.json`。
4. 把 `PASS` / `WARNING` batch 视为完成。
5. 如果信任 resolution 文件，存在同名 `batch-XX.resolution.md` 的失败 batch 也视为完成。
6. 生成 remaining task 文件。
7. 写 `supervisor-status.md/json`。
8. 如果是 `--plan-only`，到此停止。
9. 否则用 remaining task 文件调用 Python runner。
10. attempt 结束后重新扫描 summary，刷新 remaining task 和 status。
11. 如果失败看起来是 runner 脚本自身故障，且启用 `--repair`，启动受限 Codex repair 会话。
12. 未完成且 attempt 次数未耗尽时继续下一轮。

## Supervisor 参数

直接 Python 入口：

```powershell
python .\scripts\standard_task_list_omx_supervisor.py [supervisor-options] -- [runner-options]
```

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--tasks-file` | `codex-tasks.txt` | 完整任务清单。 |
| `--runner-script` | `scripts/standard_task_list_omx.py` | supervisor 调用的 runner。 |
| `--observed-output-root` | 可重复；默认 `target/codex-runs/standard-task-list-omx` | 历史 runner 输出目录。 |
| `--supervisor-output-root` | `target/codex-runs/standard-task-list-omx-supervisor` | supervisor 状态、日志和 remaining 文件输出目录。 |
| `--max-attempts` | `3` | 最多 runner attempt 次数，必须大于 0。 |
| `--start-at-task` | 空 | 从匹配任务开始生成 remaining。 |
| `--start-after-task` | 空 | 从匹配任务之后生成 remaining。 |
| `--no-trust-resolution-files` | 关闭 | 不把 `batch-XX.resolution.md` 当作人工解决证明。 |
| `--plan-only` | 关闭 | 只写 status 和 remaining tasks，不运行 runner。 |
| `--repair` | 关闭 | runner 自身故障时允许启动 Codex repair。 |
| `--repair-model` | `gpt-5.5` | repair 会话模型。 |
| `--repair-reasoning-effort` | `medium` | repair 会话 reasoning effort。 |
| `--repair-timeout-seconds` | `900` | repair 会话超时。 |
| `--codex` | `codex` | repair 使用的 Codex 可执行文件。 |
| `runner_args` | 空 | `--` 后传给 runner 的参数。 |

Supervisor 会强制控制 runner 的 `--tasks-file` 和 `--output-root`。即使用户在 `--` 后传入这两个参数，也会被 `strip_runner_file_args()` 移除，以免误跑完整任务清单或覆盖输出目录。

## Supervisor 输出

默认输出根目录：

```text
target/codex-runs/standard-task-list-omx-supervisor/
```

常见产物：

| 文件或目录 | 内容 |
| --- | --- |
| `supervisor-status.md` | 当前完成数、剩余数、未解决 batch、remaining tasks。 |
| `supervisor-status.json` | 机器可读 supervisor 状态。 |
| `<timestamp>/remaining-attempt-XX.tasks.txt` | 某轮 attempt 开始前生成的剩余任务。 |
| `<timestamp>/remaining-after-attempt-XX.tasks.txt` | 某轮 attempt 结束后刷新出来的剩余任务。 |
| `<timestamp>/attempt-XX/` | 本轮 runner 的 output root。 |
| `<timestamp>/supervisor-attempt-XX.log` | 本轮 runner stdout/stderr 合并日志。 |
| `<timestamp>/attempt-XX/supervisor-repair-*.log` | 可选 repair 会话日志。 |

## 完成判断和 resolution 文件

Supervisor 的完成判断由 `collect_progress()` 实现：

- `PASS`：完成。
- `WARNING`：完成。
- `BLOCKER`：未完成。
- `ERROR`：未完成。
- 失败 batch 如果有同名 `batch-XX.resolution.md`，且未启用 `--no-trust-resolution-files`，视为完成。

Resolution 文件路径示例：

```text
target/codex-runs/standard-task-list-omx/batch-07.summary.json
target/codex-runs/standard-task-list-omx/batch-07.resolution.md
```

使用 resolution 文件时，应在文件中写清楚为什么该失败 batch 已被后续人工验证或 follow-up 任务解决。Supervisor 只检查文件是否存在，不解析内容。

## Runner 故障和任务失败的区别

`supervisor.looks_like_runner_failure()` 用来判断非零退出是否像 runner/脚本层故障。

倾向于 runner 故障的信号：

- attempt log 中出现 Python traceback、`SyntaxError`、`NameError`、`TypeError`、`ValueError`、`RuntimeError`。
- 出现 argparse 或 PowerShell 参数绑定错误。
- log 中出现 `Runner not found:` 或 `Task file not found:`。
- 本轮 attempt 没有生成任何 `batch-*.summary.json`。
- 完成任务数没有增加，且没有留下 unresolved batch summary。

不算 runner 故障的典型情况：

- runner 正常写出 `BLOCKER` summary。
- runner 正常写出 `ERROR` summary。
- guard、cargo 验证、agent last-message 明确报告失败。

这些情况应先读 batch summary、failure log 和 last-message，而不是启动 repair。

## Repair 会话

启用：

```powershell
python .\scripts\standard_task_list_omx_supervisor.py --repair -- --skip-final-validation
```

当失败被判定为 runner/脚本层故障时，`repair_runner()` 会启动：

```powershell
codex exec --cd <repo> --sandbox danger-full-access --model <model> --config model_reasoning_effort="<effort>" -
```

Repair prompt 会限制修复范围：

- 只检查 runner logs、supervisor report、脚本测试和文档。
- 只修改 orchestration scripts、tests、docs。
- 不修改 Rust feature/application code。
- 不运行完整任务列表作为验证。
- 修改后运行 focused Python tests。

如果 `codex` 无法启动，repair 不会让 supervisor 崩溃；它会写 `supervisor-repair-*.log` 并返回失败退出码。

## 常用命令

预览 runner batch，不启动 Codex：

```powershell
python .\scripts\standard_task_list_omx.py --dry-run
```

执行 runner，但不提交、不跑最终长验证：

```powershell
python .\scripts\standard_task_list_omx.py --skip-commit --skip-final-validation
```

只查看 supervisor 认为还剩哪些任务：

```powershell
python .\scripts\standard_task_list_omx_supervisor.py --plan-only
```

续跑剩余任务，并跳过最终长验证：

```powershell
python .\scripts\standard_task_list_omx_supervisor.py -- --skip-final-validation
```

从某个任务之后续跑：

```powershell
python .\scripts\standard_task_list_omx_supervisor.py `
  --start-after-task "api-models-02" `
  -- --skip-final-validation
```

临时观察多个输出目录：

```powershell
python .\scripts\standard_task_list_omx_supervisor.py `
  --observed-output-root target/codex-runs/standard-task-list-omx `
  --observed-output-root target/codex-runs/standard-task-list-omx-supervisor `
  --plan-only
```

## 测试

Focused Python 测试：

```powershell
python -m unittest scripts.tests.test_standard_task_list_omx
```

这些测试覆盖：

- 任务文件读取。
- checkpoint batch 切分。
- 历史 oversized Rust 文件 guard。
- blocker-risk review 结论解析和非中断状态。
- batch summary 状态写入。
- supervisor remaining task 计算。
- resolution 文件完成判断。
- runner failure 分类。
- failed attempt 后 supervisor status 刷新。
- 无效 UTF-8 输出替换。
- repair runner 无法启动 Codex 时的错误记录。

文档或脚本改动后，至少运行 focused Python 测试。若改动影响最终验证逻辑，再运行 runner dry run 和相关 cargo 命令。

## 维护注意事项

- 不要在子任务 agent 内直接 commit；commit 由 runner 统一负责。
- 不要让 supervisor 覆盖用户传入的完整任务清单；它必须始终生成 remaining tasks。
- 修改 batch 状态语义时，同时检查 `artifacts.batch_status()` 和 `supervisor.COMPLETE_STATUSES`。
- 修改 summary JSON 字段时，确认 supervisor 和测试仍能读取历史产物。
- 修改 guard 文案时，确认 `agent_line_findings()` 和相关测试不会误判。
- 修改 PowerShell wrapper 参数时，同步检查 Python parser 和 `docs/scripts/README.md`。
- 不要把 `target/codex-runs/...` 产物提交为源码文档，除非明确需要保留一次运行证据。

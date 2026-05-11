from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Sequence

from .common import now_iso, resolve_repo_path, write_lines
from .tasks import load_task_list


DEFAULT_TASKS_FILE = "codex-tasks.txt"
DEFAULT_RUNNER_SCRIPT = "scripts/standard_task_list_omx.py"
DEFAULT_OBSERVED_OUTPUT_ROOT = "target/codex-runs/standard-task-list-omx"
DEFAULT_SUPERVISOR_OUTPUT_ROOT = "target/codex-runs/standard-task-list-omx-supervisor"
COMPLETE_STATUSES = {"PASS", "WARNING"}
RUNNER_FAILURE_MARKERS = (
    "Traceback (most recent call last)",
    "SyntaxError:",
    "NameError:",
    "TypeError:",
    "ValueError:",
    "RuntimeError:",
    "argparse.ArgumentError",
    "error: unrecognized arguments",
    "Runner not found:",
    "Task file not found:",
    "The term",
    "is not recognized as",
    "Cannot bind parameter",
    "ParameterBindingException",
)


@dataclass(frozen=True)
class ObservedBatch:
    path: Path
    status: str
    tasks: tuple[str, ...]
    resolved_by_file: bool = False


@dataclass(frozen=True)
class ObservedProgress:
    completed_keys: frozenset[str]
    completed_tasks: tuple[str, ...]
    unresolved_batches: tuple[ObservedBatch, ...]
    observed_batches: tuple[ObservedBatch, ...]


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def task_key(task: str) -> str:
    return " ".join(task.split()).casefold()


def read_json(path: Path, default: Any) -> Any:
    if not path.exists():
        return default
    text = path.read_text(encoding="utf-8-sig", errors="replace").strip()
    if not text:
        return default
    return json.loads(text)


def summary_resolution_path(summary_path: Path) -> Path:
    return summary_path.with_name(summary_path.name.replace(".summary.json", ".resolution.md"))


def load_summary(path: Path, trust_resolution_files: bool) -> ObservedBatch | None:
    try:
        record = read_json(path, {})
    except json.JSONDecodeError:
        return None
    if not isinstance(record, dict):
        return None

    raw_tasks = record.get("tasks", [])
    if isinstance(raw_tasks, str):
        tasks = (raw_tasks,)
    elif isinstance(raw_tasks, list):
        tasks = tuple(str(item) for item in raw_tasks if str(item).strip())
    else:
        tasks = ()
    if not tasks:
        return None

    status = str(record.get("status", "")).upper()
    resolved_by_file = trust_resolution_files and summary_resolution_path(path).exists()
    return ObservedBatch(path=path, status=status, tasks=tasks, resolved_by_file=resolved_by_file)


def discover_batch_summaries(output_roots: Sequence[Path], trust_resolution_files: bool) -> list[ObservedBatch]:
    summaries: list[ObservedBatch] = []
    seen: set[Path] = set()
    for root in output_roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("batch-*.summary.json")):
            resolved = path.resolve()
            if resolved in seen:
                continue
            seen.add(resolved)
            summary = load_summary(path, trust_resolution_files)
            if summary is not None:
                summaries.append(summary)
    return summaries


def collect_progress(output_roots: Sequence[Path], trust_resolution_files: bool = True) -> ObservedProgress:
    summaries = discover_batch_summaries(output_roots, trust_resolution_files)
    completed: dict[str, str] = {}
    unresolved: list[ObservedBatch] = []

    for summary in summaries:
        is_complete = summary.status in COMPLETE_STATUSES or summary.resolved_by_file
        if is_complete:
            for task in summary.tasks:
                completed[task_key(task)] = task
        else:
            unresolved.append(summary)

    effective_unresolved = [
        summary
        for summary in unresolved
        if any(task_key(task) not in completed for task in summary.tasks)
    ]
    return ObservedProgress(
        completed_keys=frozenset(completed),
        completed_tasks=tuple(completed[key] for key in sorted(completed)),
        unresolved_batches=tuple(effective_unresolved),
        observed_batches=tuple(summaries),
    )


def remaining_tasks(
    all_tasks: Sequence[str],
    completed_keys: set[str] | frozenset[str],
    *,
    start_at: str = "",
    start_after: str = "",
) -> list[str]:
    start_index = selected_start_index(all_tasks, start_at=start_at, start_after=start_after)
    candidates = list(all_tasks[start_index:])
    return [task for task in candidates if task_key(task) not in completed_keys]


def selected_start_index(tasks: Sequence[str], *, start_at: str, start_after: str) -> int:
    if start_at and start_after:
        raise ValueError("--start-at-task and --start-after-task are mutually exclusive.")
    if start_at:
        return find_task_index(tasks, start_at)
    if start_after:
        return find_task_index(tasks, start_after) + 1
    return 0


def find_task_index(tasks: Sequence[str], needle: str) -> int:
    needle_key = task_key(needle)
    for index, task in enumerate(tasks):
        task_normalized = task_key(task)
        if needle_key == task_normalized or needle_key in task_normalized:
            return index
    raise ValueError(f"Task selector did not match any task: {needle}")


def write_remaining_tasks(path: Path, tasks: Sequence[str]) -> None:
    write_lines(path, tasks)


def write_supervisor_report(
    output_root: Path,
    tasks_file: Path,
    all_tasks: Sequence[str],
    progress: ObservedProgress,
    remaining: Sequence[str],
    remaining_file: Path,
    observed_roots: Sequence[Path],
) -> None:
    output_root.mkdir(parents=True, exist_ok=True)
    unresolved_records = [
        {
            "path": str(batch.path.resolve()),
            "status": batch.status,
            "tasks": list(batch.tasks),
            "resolved_by_file": batch.resolved_by_file,
        }
        for batch in progress.unresolved_batches
    ]
    record = {
        "written_at": now_iso(),
        "tasks_file": str(tasks_file.resolve()),
        "total_tasks": len(all_tasks),
        "completed_tasks": len(progress.completed_keys),
        "remaining_tasks": len(remaining),
        "remaining_tasks_file": str(remaining_file.resolve()),
        "observed_batch_count": len(progress.observed_batches),
        "observed_roots": [str(root.resolve()) for root in observed_roots],
        "unresolved_batches": unresolved_records,
    }
    (output_root / "supervisor-status.json").write_text(
        json.dumps(record, indent=4, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    lines = [
        "# Standard task list supervisor",
        "",
        f"Written: {record['written_at']}",
        f"Tasks file: {record['tasks_file']}",
        f"Tasks complete: {record['completed_tasks']} / {record['total_tasks']}",
        f"Remaining tasks: {record['remaining_tasks']}",
        f"Remaining tasks file: {record['remaining_tasks_file']}",
        f"Observed batch summaries: {record['observed_batch_count']}",
        "",
        "## Unresolved batches",
    ]
    if unresolved_records:
        for item in unresolved_records:
            lines.append(f"- {item['status']}: {item['path']}")
    else:
        lines.append("- (none)")
    lines.extend(["", "## Remaining tasks"])
    lines.extend(["- (none)"] if not remaining else [f"- {task}" for task in remaining])
    write_lines(output_root / "supervisor-status.md", lines)


def write_progress_snapshot(
    supervisor_root: Path,
    tasks_file: Path,
    all_tasks: Sequence[str],
    progress: ObservedProgress,
    remaining_file: Path,
    observed_roots: Sequence[Path],
    *,
    start_at: str = "",
    start_after: str = "",
) -> list[str]:
    remaining = remaining_tasks(
        all_tasks,
        progress.completed_keys,
        start_at=start_at,
        start_after=start_after,
    )
    write_remaining_tasks(remaining_file, remaining)
    write_supervisor_report(
        supervisor_root,
        tasks_file,
        all_tasks,
        progress,
        remaining,
        remaining_file,
        observed_roots,
    )
    return remaining


def strip_runner_file_args(args: Sequence[str]) -> list[str]:
    return strip_options(args, {"--tasks-file", "--output-root"})


def strip_options(args: Sequence[str], options_with_values: set[str]) -> list[str]:
    result: list[str] = []
    skip_next = False
    for arg in args:
        if skip_next:
            skip_next = False
            continue
        if arg in options_with_values:
            skip_next = True
            continue
        if any(arg.startswith(option + "=") for option in options_with_values):
            continue
        result.append(arg)
    return result


def runner_command(
    root: Path,
    runner_script: Path,
    remaining_file: Path,
    attempt_output_root: Path,
    runner_args: Sequence[str],
) -> list[str]:
    passthrough = strip_runner_file_args(runner_args)
    return [
        sys.executable,
        str(runner_script),
        "--tasks-file",
        str(remaining_file),
        "--output-root",
        str(attempt_output_root),
        *passthrough,
    ]


def run_attempt(command: Sequence[str], root: Path, log_file: Path) -> int:
    print()
    print("=== Supervised standard-task-list attempt ===")
    print(" ".join(str(part) for part in command))
    log_file.parent.mkdir(parents=True, exist_ok=True)
    with log_file.open("w", encoding="utf-8", errors="replace") as log:
        with subprocess.Popen(
            list(command),
            cwd=str(root),
            text=True,
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        ) as process:
            assert process.stdout is not None
            for line in process.stdout:
                print(line, end="")
                log.write(line)
                log.flush()
            return process.wait()


def looks_like_runner_failure(
    exit_code: int,
    attempt_output_root: Path,
    log_file: Path,
    before: ObservedProgress,
    after: ObservedProgress,
) -> bool:
    if exit_code == 0:
        return False
    log_text = log_file.read_text(encoding="utf-8", errors="replace") if log_file.exists() else ""
    if any(marker in log_text for marker in RUNNER_FAILURE_MARKERS):
        return True
    if not list(attempt_output_root.glob("batch-*.summary.json")):
        return True
    if len(after.completed_keys) <= len(before.completed_keys) and not after.unresolved_batches:
        return True
    return False


def repair_runner(
    root: Path,
    runner_script: Path,
    supervisor_report: Path,
    attempt_log: Path,
    attempt_output_root: Path,
    codex: str,
    model: str,
    reasoning_effort: str,
    timeout_seconds: int,
) -> int:
    repair_log = attempt_output_root / f"supervisor-repair-{int(time.time())}.log"
    codex_executable = shutil.which(codex) or codex
    prompt = f"""The standard-task-list-omx runner exited before all tasks completed.

Inspect the runner logs and supervisor report, then patch only the orchestration
scripts/tests/docs needed to make the runner resume safely.

Repository: {root}
Runner script: {runner_script}
Supervisor report: {supervisor_report}
Attempt log: {attempt_log}
Attempt output root: {attempt_output_root}

Constraints:
- Preserve unrelated worktree changes.
- Do not edit Rust feature/application code.
- Prefer small, reviewable fixes under scripts/ and docs/scripts/.
- Keep scripts/standard_task_list_omx.py and run-standard-task-list-omx.ps1 working.
- Do not run the full task list as validation.
- Run focused Python tests for the runner/supervisor after editing.

Return a concise note with changed files, verification, and remaining risk.
"""
    command = [
        codex_executable,
        "exec",
        "--cd",
        str(root),
        "--sandbox",
        "danger-full-access",
        "--model",
        model,
        "--config",
        f'model_reasoning_effort="{reasoning_effort}"',
        "-",
    ]
    print()
    print("=== Supervisor repair session ===")
    print(" ".join(command))
    with repair_log.open("w", encoding="utf-8", errors="replace") as log:
        try:
            process = subprocess.Popen(
                command,
                cwd=str(root),
                text=True,
                encoding="utf-8",
                errors="replace",
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
            )
        except OSError as exc:
            message = f"ERROR: failed to start codex repair command ({codex_executable}): {exc}"
            print(message)
            log.write(message + "\n")
            return 1
        assert process.stdin is not None
        assert process.stdout is not None
        process.stdin.write(prompt)
        process.stdin.close()
        try:
            for line in process.stdout:
                print(line, end="")
                log.write(line)
                log.flush()
            return process.wait(timeout=timeout_seconds if timeout_seconds > 0 else None)
        except subprocess.TimeoutExpired:
            process.kill()
            return 124


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Supervise standard-task-list-omx and resume incomplete task queues.",
        epilog="Arguments after -- are passed to scripts/standard_task_list_omx.py.",
    )
    parser.add_argument("--tasks-file", default=DEFAULT_TASKS_FILE)
    parser.add_argument("--runner-script", default=DEFAULT_RUNNER_SCRIPT)
    parser.add_argument(
        "--observed-output-root",
        action="append",
        default=[],
        help="Existing runner output root to inspect. May be passed more than once.",
    )
    parser.add_argument("--supervisor-output-root", default=DEFAULT_SUPERVISOR_OUTPUT_ROOT)
    parser.add_argument("--max-attempts", type=int, default=3)
    parser.add_argument("--start-at-task", default="")
    parser.add_argument("--start-after-task", default="")
    parser.add_argument(
        "--no-trust-resolution-files",
        action="store_true",
        help="Do not count batch-XX.resolution.md as resolving a failed summary.",
    )
    parser.add_argument("--plan-only", action="store_true", help="Write status and remaining tasks without running Codex.")
    parser.add_argument("--repair", action="store_true", help="Allow a bounded Codex repair session for runner failures.")
    parser.add_argument("--repair-model", default="gpt-5.5")
    parser.add_argument("--repair-reasoning-effort", default="medium")
    parser.add_argument("--repair-timeout-seconds", type=int, default=900)
    parser.add_argument("--codex", default="codex")
    parser.add_argument("runner_args", nargs=argparse.REMAINDER)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.runner_args and args.runner_args[0] == "--":
        args.runner_args = args.runner_args[1:]
    if args.max_attempts <= 0:
        parser.error("--max-attempts must be greater than zero.")
    if args.repair:
        codex_executable = shutil.which(args.codex)
        if codex_executable is None:
            parser.error(f"codex executable not found: {args.codex}")
        args.codex = codex_executable

    root = repo_root()
    tasks_file = resolve_repo_path(root, args.tasks_file)
    runner_script = resolve_repo_path(root, args.runner_script)
    supervisor_root = resolve_repo_path(root, args.supervisor_output_root)
    default_observed = resolve_repo_path(root, DEFAULT_OBSERVED_OUTPUT_ROOT)
    observed_roots = [resolve_repo_path(root, value) for value in args.observed_output_root] or [default_observed]
    observed_roots.append(supervisor_root)
    trust_resolution_files = not args.no_trust_resolution_files

    if not runner_script.exists():
        parser.error(f"runner script not found: {runner_script}")

    all_tasks = load_task_list(tasks_file)
    session_root = supervisor_root / datetime.now().strftime("%Y%m%d-%H%M%S")
    supervisor_root.mkdir(parents=True, exist_ok=True)
    session_root.mkdir(parents=True, exist_ok=True)

    last_exit_code = 0
    for attempt in range(0, args.max_attempts + 1):
        progress = collect_progress(observed_roots, trust_resolution_files=trust_resolution_files)
        remaining_file = session_root / f"remaining-attempt-{attempt + 1:02d}.tasks.txt"
        remaining = write_progress_snapshot(
            supervisor_root,
            tasks_file,
            all_tasks,
            progress,
            remaining_file,
            observed_roots,
            start_at=args.start_at_task,
            start_after=args.start_after_task,
        )

        print(
            f"Observed {len(progress.completed_keys)} / {len(all_tasks)} complete task(s); "
            f"{len(remaining)} remaining."
        )
        print(f"Remaining tasks file: {remaining_file}")
        if not remaining:
            return 0 if last_exit_code == 0 else last_exit_code
        if args.plan_only:
            return 0
        if attempt >= args.max_attempts:
            print(f"Stopped after {args.max_attempts} attempt(s).")
            return last_exit_code or 1

        attempt_number = attempt + 1
        attempt_output_root = session_root / f"attempt-{attempt_number:02d}"
        log_file = session_root / f"supervisor-attempt-{attempt_number:02d}.log"
        before = progress
        command = runner_command(root, runner_script, remaining_file, attempt_output_root, args.runner_args)
        last_exit_code = run_attempt(command, root, log_file)
        after = collect_progress(observed_roots, trust_resolution_files=trust_resolution_files)
        after_remaining_file = session_root / f"remaining-after-attempt-{attempt_number:02d}.tasks.txt"
        after_remaining = write_progress_snapshot(
            supervisor_root,
            tasks_file,
            all_tasks,
            after,
            after_remaining_file,
            observed_roots,
            start_at=args.start_at_task,
            start_after=args.start_after_task,
        )
        print(
            f"After attempt {attempt_number}: observed {len(after.completed_keys)} / {len(all_tasks)} "
            f"complete task(s); {len(after_remaining)} remaining."
        )
        print(f"Resume tasks file: {after_remaining_file}")

        if last_exit_code != 0:
            print(f"Attempt {attempt_number} exited with {last_exit_code}.")
            if args.repair and looks_like_runner_failure(last_exit_code, attempt_output_root, log_file, before, after):
                repair_exit = repair_runner(
                    root,
                    runner_script,
                    supervisor_root / "supervisor-status.md",
                    log_file,
                    attempt_output_root,
                    args.codex,
                    args.repair_model,
                    args.repair_reasoning_effort,
                    args.repair_timeout_seconds,
                )
                if repair_exit != 0:
                    print(f"Supervisor repair failed with exit code {repair_exit}.")
                    return repair_exit
                continue

            print("Attempt failed in task/guard validation rather than runner orchestration; inspect supervisor-status.md.")
            return last_exit_code

    return last_exit_code or 1

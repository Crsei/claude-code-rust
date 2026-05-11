from __future__ import annotations

import argparse
import subprocess
import sys
from datetime import datetime
from pathlib import Path
from typing import Sequence

from .artifacts import batch_diagnostic_lines, write_batch_artifacts, write_failure_record
from .blocker_review import (
    apply_blocker_review_result,
    blocker_risk_findings,
    review_blockers,
    should_review_blockers,
)
from .commit import commit_batch
from .common import GLOBAL_CONTRACT, path_key, resolve_repo_path, write_lines
from .git_state import batch_changed_paths, git_changed_paths, path_signature_map
from .guards import agent_reported_findings, has_blocking_finding, invoke_diff_guards
from .tasks import load_task_list, next_batch_size
from .validation import invoke_final_validation, write_run_report


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run standard task list batches.")
    parser.add_argument(
        "--tasks-file",
        default="codex-tasks.txt",
    )
    parser.add_argument("--runner", default="scripts/codex-task-sequence.ps1")
    parser.add_argument("--codex", default="codex")
    parser.add_argument("--work-dir", default=".")
    parser.add_argument(
        "--output-root",
        default="target/codex-runs/standard-task-list-omx",
    )
    parser.add_argument("--model", choices=["gpt-5.5"], default="gpt-5.5")
    parser.add_argument("--reasoning-effort", choices=["medium"], default="medium")
    parser.add_argument("--sandbox", default="danger-full-access")
    parser.add_argument("--initial-batch-size", type=int, default=1)
    parser.add_argument("--min-batch-size", type=int, default=1)
    parser.add_argument("--max-batch-size", type=int, default=3)
    parser.add_argument("--warn-rust-file-lines", type=int, default=500)
    parser.add_argument("--max-rust-file-lines", type=int, default=2000)
    parser.add_argument("--max-files-per-batch", type=int, default=12)
    parser.add_argument("--max-per-file-diff-lines", type=int, default=600)
    parser.add_argument("--max-oversized-rust-files-before-blocker", type=int, default=3)
    parser.add_argument("--skip-blocker-review", action="store_true")
    parser.add_argument("--blocker-review-sandbox", default="read-only")
    parser.add_argument("--blocker-review-timeout-seconds", type=int, default=900)
    baseline_group = parser.add_mutually_exclusive_group()
    baseline_group.add_argument(
        "--commit-baseline-dirty-changes",
        dest="commit_baseline_dirty_changes",
        action="store_true",
    )
    baseline_group.add_argument(
        "--no-commit-baseline-dirty-changes",
        dest="commit_baseline_dirty_changes",
        action="store_false",
    )
    parser.set_defaults(commit_baseline_dirty_changes=True)
    parser.add_argument("--continue-on-error", action="store_true")
    parser.add_argument("--skip-commit", action="store_true")
    parser.add_argument("--skip-final-validation", action="store_true")
    parser.add_argument("--dry-run", action="store_true")
    return parser


def run(args: argparse.Namespace) -> int:
    repo_root = Path(__file__).resolve().parents[2]
    tasks_file = resolve_repo_path(repo_root, args.tasks_file)
    runner = resolve_repo_path(repo_root, args.runner)
    work_dir = resolve_repo_path(repo_root, args.work_dir)
    output_root = resolve_repo_path(repo_root, args.output_root)

    if not runner.exists():
        raise FileNotFoundError(f"Runner not found: {runner}")
    if args.max_oversized_rust_files_before_blocker < 1:
        raise RuntimeError("--max-oversized-rust-files-before-blocker must be greater than zero")
    if args.blocker_review_timeout_seconds < 0:
        raise RuntimeError("--blocker-review-timeout-seconds cannot be negative")

    tasks = load_task_list(tasks_file)
    if not tasks:
        raise RuntimeError(f"Task file is empty: {tasks_file}")

    state = RunState(
        batch_size=max(args.min_batch_size, min(args.initial_batch_size, args.max_batch_size)),
        started_at=datetime.now().astimezone(),
    )
    output_root.mkdir(parents=True, exist_ok=True)

    baseline_dirty_paths = git_changed_paths(repo_root)
    baseline_dirty = {path_key(path) for path in baseline_dirty_paths}
    baseline_signatures = path_signature_map(repo_root, baseline_dirty_paths)

    while state.index < len(tasks):
        try:
            run_batch(
                args,
                repo_root,
                runner,
                work_dir,
                output_root,
                tasks,
                baseline_dirty,
                baseline_signatures,
                state,
            )
        except Exception as exc:  # noqa: BLE001 - write an interruption report before returning.
            state.had_failure = True
            state.stopped_early = True
            state.stop_reason = f"Runner interrupted: {exc}"
        write_run_report(
            repo_root,
            output_root,
            state.started_at,
            datetime.now().astimezone(),
            len(tasks),
            state.completed_task_count,
            state.stopped_early,
            state.stop_reason,
            state.validation_results,
            args.dry_run,
        )
        if state.stopped_early:
            break

    if not args.dry_run and not args.skip_final_validation:
        state.validation_results = invoke_final_validation(repo_root, output_root)
        if any(int(result["exit_code"]) != 0 for result in state.validation_results):
            state.had_failure = True
            if not state.stop_reason:
                state.stop_reason = "Final validation failed"

    write_run_report(
        repo_root,
        output_root,
        state.started_at,
        datetime.now().astimezone(),
        len(tasks),
        state.completed_task_count,
        state.stopped_early,
        state.stop_reason,
        state.validation_results,
        args.dry_run,
    )

    print()
    print(f"Standard task list run finished. Artifacts: {output_root}")
    return 1 if state.had_failure else 0


class RunState:
    def __init__(self, batch_size: int, started_at: datetime) -> None:
        self.batch_size = batch_size
        self.started_at = started_at
        self.clean_batch_count = 0
        self.batch_number = 0
        self.index = 0
        self.completed_task_count = 0
        self.had_failure = False
        self.stopped_early = False
        self.stop_reason = ""
        self.validation_results: list[dict[str, object]] = []


def run_batch(
    args: argparse.Namespace,
    repo_root: Path,
    runner: Path,
    work_dir: Path,
    output_root: Path,
    tasks: Sequence[str],
    baseline_dirty: set[str],
    baseline_signatures: dict[str, str],
    state: RunState,
) -> None:
    state.batch_number += 1
    batch_label = f"{state.batch_number:02d}"
    remaining = len(tasks) - state.index
    take = next_batch_size(tasks, state.index, state.batch_size, remaining)
    batch_tasks = list(tasks[state.index : state.index + take])
    batch_tasks_file = output_root / f"batch-{batch_label}.tasks.txt"
    batch_output_dir = output_root / f"batch-{batch_label}"
    batch_output_dir.mkdir(parents=True, exist_ok=True)

    write_lines(batch_tasks_file, [f"{GLOBAL_CONTRACT} Task: {task}" for task in batch_tasks])
    print()
    print(
        f"=== Standard task list batch {batch_label} "
        f"({take} task(s), batch size {state.batch_size}) ==="
    )
    for task in batch_tasks:
        print(f"- {task}")

    if args.dry_run:
        print(f"DryRun: would invoke {args.codex} through {runner}")
        state.completed_task_count += take
        state.index += take
        return

    exit_code = invoke_runner_process(
        runner_args(args, runner, work_dir, batch_tasks_file, batch_output_dir),
        repo_root,
    )
    current_changed = git_changed_paths(repo_root)
    changed_for_batch = batch_changed_paths(
        repo_root,
        current_changed,
        baseline_dirty,
        baseline_signatures,
        args.commit_baseline_dirty_changes,
    )
    violations = invoke_diff_guards(
        repo_root,
        changed_for_batch,
        args.warn_rust_file_lines,
        args.max_rust_file_lines,
        args.max_files_per_batch,
        args.max_per_file_diff_lines,
        args.max_oversized_rust_files_before_blocker,
    )
    violations.extend(agent_reported_findings(batch_output_dir))
    if blocker_risk_findings(violations):
        violations.append(f"WARNING: blocker risk starts at task: {batch_tasks[0]}")
    if not args.skip_blocker_review and should_review_blockers(exit_code, violations):
        review_result = review_blockers(
            repo_root=repo_root,
            batch_number=state.batch_number,
            tasks=batch_tasks,
            changed_paths=changed_for_batch,
            findings=violations,
            batch_output_dir=batch_output_dir,
            codex=args.codex,
            model=args.model,
            reasoning_effort=args.reasoning_effort,
            sandbox=args.blocker_review_sandbox,
            timeout_seconds=args.blocker_review_timeout_seconds,
        )
        violations = apply_blocker_review_result(violations, review_result)
    write_batch_artifacts(
        output_root,
        state.batch_number,
        batch_tasks,
        exit_code,
        changed_for_batch,
        violations,
        batch_output_dir,
    )

    if exit_code == 0 and not has_blocking_finding(violations):
        handle_green_batch(args, repo_root, output_root, changed_for_batch, batch_tasks, batch_output_dir, violations, state)
    else:
        handle_failed_batch(args, output_root, batch_tasks, exit_code, violations, state)

    if not state.stopped_early:
        state.completed_task_count += take
        state.index += take


def runner_args(
    args: argparse.Namespace,
    runner: Path,
    work_dir: Path,
    batch_tasks_file: Path,
    batch_output_dir: Path,
) -> list[str]:
    command = [
        "powershell",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        str(runner),
        "-TasksFile",
        str(batch_tasks_file),
        "-Codex",
        args.codex,
        "-WorkDir",
        str(work_dir),
        "-OutputDir",
        str(batch_output_dir),
        "-Model",
        args.model,
        "-ReasoningEffort",
        args.reasoning_effort,
        "-Sandbox",
        args.sandbox,
    ]
    if args.continue_on_error:
        command.append("-ContinueOnError")
    return command


def invoke_runner_process(args: Sequence[str], repo_root: Path) -> int:
    proc = subprocess.run(list(args), cwd=str(repo_root))
    return proc.returncode


def handle_green_batch(
    args: argparse.Namespace,
    repo_root: Path,
    output_root: Path,
    changed_for_batch: Sequence[str],
    batch_tasks: Sequence[str],
    batch_output_dir: Path,
    violations: list[str],
    state: RunState,
) -> None:
    batch_label = f"{state.batch_number:02d}"
    if not args.skip_commit:
        try:
            commit_batch(
                repo_root,
                changed_for_batch,
                state.batch_number,
                output_root,
                args.commit_baseline_dirty_changes,
            )
        except Exception as exc:  # noqa: BLE001 - runner must record commit failures.
            commit_finding = f"ERROR: commit failed for batch {batch_label} - {exc}"
            violations.append(commit_finding)
            write_batch_artifacts(
                output_root,
                state.batch_number,
                batch_tasks,
                1,
                changed_for_batch,
                violations,
                batch_output_dir,
            )
            write_failure_record(output_root, state.batch_number, batch_tasks, [commit_finding])
            state.had_failure = True
            state.stop_reason = commit_finding
            if not args.continue_on_error:
                state.stopped_early = True
                return
    else:
        print(f"SkipCommit enabled; not committing batch {batch_label}.")

    if not state.had_failure:
        state.clean_batch_count += 1
        if state.clean_batch_count >= 2 and state.batch_size < args.max_batch_size:
            state.batch_size += 1
            state.clean_batch_count = 0


def handle_failed_batch(
    args: argparse.Namespace,
    output_root: Path,
    batch_tasks: Sequence[str],
    exit_code: int,
    violations: Sequence[str],
    state: RunState,
) -> None:
    batch_label = f"{state.batch_number:02d}"
    diagnostics = batch_diagnostic_lines(exit_code, violations)
    write_failure_record(output_root, state.batch_number, batch_tasks, diagnostics)
    state.had_failure = True
    state.clean_batch_count = 0
    state.batch_size = max(args.min_batch_size, state.batch_size - 1)
    summary_path = output_root / f"batch-{batch_label}.summary.md"
    state.stop_reason = f"Batch {batch_label} failed; see {summary_path}"
    print(f"Batch {batch_label} did not pass. Summary: {summary_path}", file=sys.stderr)
    for diagnostic in diagnostics:
        print(f"- {diagnostic}", file=sys.stderr)
    if not args.continue_on_error:
        state.stopped_early = True


def main(argv: Sequence[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except Exception as exc:  # noqa: BLE001 - CLI should return a concise failure.
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1

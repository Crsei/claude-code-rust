from __future__ import annotations

import json
import subprocess
import sys
from collections import Counter
from datetime import datetime
from pathlib import Path
from typing import Sequence

from .common import git_lines, now_iso, write_lines


def invoke_logged_cargo_command(repo_root: Path, cargo_args: Sequence[str], log_path: Path) -> int:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        proc = subprocess.run(
            ["cargo", *cargo_args],
            cwd=str(repo_root),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as exc:
        message = f"ERROR: failed to start cargo: {exc}"
        with log_path.open("a", encoding="utf-8") as fh:
            fh.write(message + "\n")
        print(message, file=sys.stderr)
        return 1

    if proc.stdout:
        print(proc.stdout, end="")
    if proc.stderr:
        print(proc.stderr, end="", file=sys.stderr)
    with log_path.open("a", encoding="utf-8") as fh:
        fh.write(proc.stdout)
        fh.write(proc.stderr)
    return proc.returncode


def invoke_final_validation(repo_root: Path, output_root: Path) -> list[dict[str, object]]:
    validation_dir = output_root / "final-validation"
    validation_dir.mkdir(parents=True, exist_ok=True)
    commands = [
        ("fmt", ["fmt", "--all", "--check"]),
        ("check", ["check", "--workspace", "--all-targets"]),
        ("clippy", ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
        ("test", ["test", "--workspace"]),
    ]
    results: list[dict[str, object]] = []
    for name, cargo_args in commands:
        log_path = validation_dir / f"{name}.log"
        display = f"cargo {' '.join(cargo_args)}"
        log_path.write_text(
            f"Command: {display}\nStarted: {now_iso()}\n\n",
            encoding="utf-8",
        )
        print()
        print(f"=== Final validation: {display} ===")
        exit_code = invoke_logged_cargo_command(repo_root, cargo_args, log_path)
        with log_path.open("a", encoding="utf-8") as fh:
            fh.write(f"Finished: {now_iso()}\n")
            fh.write(f"Exit code: {exit_code}\n")
        results.append(
            {
                "name": name,
                "command": display,
                "exit_code": exit_code,
                "log": str(log_path.resolve()),
            }
        )

    (validation_dir / "summary.json").write_text(
        json.dumps(results, indent=4, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    lines = ["# Final validation", ""]
    for result in results:
        status = "PASS" if result["exit_code"] == 0 else "ERROR"
        lines.append(f"- {status}: `{result['command']}` -> {result['log']}")
    write_lines(validation_dir / "summary.md", lines)
    return results


def write_run_report(
    repo_root: Path,
    output_root: Path,
    started_at: datetime,
    ended_at: datetime,
    total_tasks: int,
    completed_tasks: int,
    stopped_early: bool,
    stop_reason: str,
    validation_results: Sequence[dict[str, object]],
    dry_run: bool,
) -> None:
    output_root.mkdir(parents=True, exist_ok=True)
    summaries = load_batch_summaries(output_root)
    status_counts = Counter(str(summary.get("status", "")) for summary in summaries)
    current_status = git_lines(repo_root, "status", "--short", allow_failure=True)[:200]
    status_count_records = [
        {"status": status, "count": count}
        for status, count in sorted(status_counts.items())
    ]
    json_path = output_root / "final-report.json"
    report_path = output_root / "final-report.md"
    record = {
        "started_at": started_at.astimezone().isoformat(),
        "ended_at": ended_at.astimezone().isoformat(),
        "dry_run": dry_run,
        "total_tasks": total_tasks,
        "completed_tasks": completed_tasks,
        "stopped_early": stopped_early,
        "stop_reason": stop_reason,
        "batch_count": len(summaries),
        "status_counts": status_count_records,
        "validation": list(validation_results),
        "git_status_sample": current_status,
    }
    json_path.write_text(json.dumps(record, indent=4, ensure_ascii=False) + "\n", encoding="utf-8")
    write_lines(
        report_path,
        report_markdown_lines(
            output_root,
            started_at,
            ended_at,
            total_tasks,
            completed_tasks,
            stopped_early,
            stop_reason,
            status_count_records,
            validation_results,
            dry_run,
            current_status,
            json_path,
        ),
    )


def load_batch_summaries(output_root: Path) -> list[dict[str, object]]:
    summaries: list[dict[str, object]] = []
    for path in sorted(output_root.glob("batch-*.summary.json")):
        try:
            summaries.append(json.loads(path.read_text(encoding="utf-8")))
        except json.JSONDecodeError:
            continue
    return summaries


def report_markdown_lines(
    output_root: Path,
    started_at: datetime,
    ended_at: datetime,
    total_tasks: int,
    completed_tasks: int,
    stopped_early: bool,
    stop_reason: str,
    status_count_records: Sequence[dict[str, object]],
    validation_results: Sequence[dict[str, object]],
    dry_run: bool,
    current_status: Sequence[str],
    json_path: Path,
) -> list[str]:
    lines = [
        "# Workspace crate extraction final report",
        "",
        f"Started: {started_at.astimezone().isoformat()}",
        f"Ended: {ended_at.astimezone().isoformat()}",
        f"Dry run: {dry_run}",
        f"Tasks completed: {completed_tasks} / {total_tasks}",
        f"Stopped early: {stopped_early}",
    ]
    if stop_reason:
        lines.append(f"Stop reason: {stop_reason}")
    lines.extend(["", "## Batch status counts"])
    if status_count_records:
        lines.extend(f"- {item['status']}: {item['count']}" for item in status_count_records)
    else:
        lines.append("- (none)")
    lines.extend(["", "## Failure log"])
    failure_path = output_root / "failures.md"
    lines.append(f"- {failure_path.resolve()}" if failure_path.exists() else "- No batch failures recorded.")
    lines.extend(["", "## Final validation"])
    if validation_results:
        for result in validation_results:
            status = "PASS" if result["exit_code"] == 0 else "ERROR"
            lines.append(f"- {status}: `{result['command']}` -> {result['log']}")
    else:
        lines.append("- Not run.")
    lines.extend(["", "## Current git status sample"])
    lines.extend(["- clean"] if not current_status else [f"- {line}" for line in current_status])
    lines.extend(["", f"JSON: {json_path.resolve()}"])
    return lines

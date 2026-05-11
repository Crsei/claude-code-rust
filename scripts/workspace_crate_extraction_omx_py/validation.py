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
            encoding="utf-8",
            errors="replace",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except OSError as exc:
        message = f"ERROR: failed to start cargo: {exc}"
        with log_path.open("a", encoding="utf-8") as fh:
            fh.write(message + "\n")
        print(message, file=sys.stderr)
        return 1

    stdout = proc.stdout or ""
    stderr = proc.stderr or ""
    if stdout:
        print(stdout, end="")
    if stderr:
        print(stderr, end="", file=sys.stderr)
    with log_path.open("a", encoding="utf-8") as fh:
        fh.write(stdout)
        fh.write(stderr)
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
    task_report_paths = write_task_execution_report(output_root, summaries)
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
        "task_execution_report": task_report_paths,
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
            task_report_paths,
            validation_results,
            dry_run,
            current_status,
            json_path,
        ),
    )


def write_task_execution_report(
    output_root: Path,
    batch_summaries: Sequence[dict[str, object]],
) -> dict[str, str]:
    records = load_task_execution_records(output_root, batch_summaries)
    json_path = output_root / "execution-report.json"
    md_path = output_root / "execution-report.md"
    json_path.write_text(json.dumps(records, indent=4, ensure_ascii=False) + "\n", encoding="utf-8")
    write_lines(md_path, task_execution_markdown_lines(records, json_path))
    return {"json": str(json_path.resolve()), "markdown": str(md_path.resolve())}


def load_task_execution_records(
    output_root: Path,
    batch_summaries: Sequence[dict[str, object]],
) -> list[dict[str, object]]:
    batch_by_number = {
        int(summary.get("batch", 0)): summary
        for summary in batch_summaries
        if str(summary.get("batch", "")).isdigit()
    }
    records: list[dict[str, object]] = []
    for path in sorted(output_root.glob("batch-*/task-*.summary.json")):
        try:
            record = json.loads(path.read_text(encoding="utf-8-sig"))
        except (json.JSONDecodeError, OSError):
            continue
        if not isinstance(record, dict):
            continue
        batch_number = batch_number_from_dir(path.parent.name)
        batch_summary = batch_by_number.get(batch_number, {})
        records.append(
            {
                "batch": batch_number,
                "batch_status": batch_summary.get("status", ""),
                "batch_summary": str((output_root / f"batch-{batch_number:02d}.summary.md").resolve())
                if batch_number > 0
                else "",
                "task_number": record.get("task_number", 0),
                "task_count": record.get("task_count", 0),
                "task": record.get("task", ""),
                "status": record.get("status", ""),
                "exit_code": record.get("exit_code", 0),
                "changed_files": string_list(record.get("changed_files", [])),
                "last_message_file": record.get("last_message_file", ""),
                "output_dir": record.get("output_dir", ""),
                "started_at": record.get("started_at", ""),
                "ended_at": record.get("ended_at", ""),
                "interruption_reason": record.get("interruption_reason", ""),
            }
        )
    return records


def batch_number_from_dir(name: str) -> int:
    prefix = "batch-"
    if not name.startswith(prefix):
        return 0
    try:
        return int(name.removeprefix(prefix))
    except ValueError:
        return 0


def task_execution_markdown_lines(records: Sequence[dict[str, object]], json_path: Path) -> list[str]:
    lines = ["# Workspace crate extraction execution report", ""]
    if not records:
        lines.extend(["No task execution summaries recorded.", "", f"JSON: {json_path.resolve()}"])
        return lines

    for record in records:
        title = f"Batch {int(record.get('batch', 0)):02d} task {record.get('task_number', 0)}"
        lines.extend([f"## {title}", ""])
        lines.append(f"- Status: {record.get('status', '')}")
        lines.append(f"- Batch status: {record.get('batch_status', '')}")
        lines.append(f"- Exit code: {record.get('exit_code', 0)}")
        interruption = str(record.get("interruption_reason", ""))
        if interruption:
            lines.append(f"- Interruption reason: {interruption}")
        lines.append(f"- Task: {record.get('task', '')}")
        lines.append(f"- Batch summary: {record.get('batch_summary', '')}")
        lines.append(f"- Last message: {record.get('last_message_file', '')}")
        lines.append(f"- Output dir: {record.get('output_dir', '')}")
        lines.append("- Changed files:")
        changed_files = list(record.get("changed_files", []))
        lines.extend(["  - (none)"] if not changed_files else [f"  - {path}" for path in changed_files])
        lines.append("")
    lines.append(f"JSON: {json_path.resolve()}")
    return lines


def string_list(value: object) -> list[str]:
    if value is None:
        return []
    if isinstance(value, list):
        return [str(item) for item in value]
    if isinstance(value, tuple):
        return [str(item) for item in value]
    return [str(value)]


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
    task_report_paths: dict[str, str],
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
    lines.extend(["", "## Task execution report"])
    lines.append(f"- Markdown: {task_report_paths.get('markdown', '')}")
    lines.append(f"- JSON: {task_report_paths.get('json', '')}")
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

from __future__ import annotations

import json
from pathlib import Path
from typing import Sequence

from .common import now_iso, write_lines
from .guards import has_blocking_finding, has_warning_finding


def batch_status(exit_code: int, violations: Sequence[str]) -> str:
    if exit_code != 0:
        return "ERROR"
    if has_blocking_finding(violations):
        return "BLOCKER"
    if has_warning_finding(violations):
        return "WARNING"
    return "PASS"


def batch_diagnostic_lines(exit_code: int, violations: Sequence[str]) -> list[str]:
    diagnostics: list[str] = []
    if exit_code != 0:
        diagnostics.append(
            f"ERROR: task runner exited with code {exit_code}; inspect task last-message files and runner logs"
        )
    diagnostics.extend(violations)
    if not diagnostics:
        diagnostics.append("PASS: runner exit code and diff guards passed")
    return diagnostics


def write_batch_artifacts(
    output_root: Path,
    batch_number: int,
    tasks: Sequence[str],
    exit_code: int,
    changed_paths: Sequence[str],
    violations: Sequence[str],
    batch_output_dir: Path,
) -> None:
    output_root.mkdir(parents=True, exist_ok=True)
    batch_label = f"{batch_number:02d}"
    last_messages = [
        str(path.resolve())
        for path in sorted(batch_output_dir.glob("task-*.last-message.txt"))
        if path.is_file()
    ]
    status = batch_status(exit_code, violations)
    diagnostics = batch_diagnostic_lines(exit_code, violations)
    json_path = output_root / f"batch-{batch_label}.summary.json"
    md_path = output_root / f"batch-{batch_label}.summary.md"
    record = {
        "batch": batch_number,
        "status": status,
        "exit_code": exit_code,
        "model": "gpt-5.5",
        "reasoning_effort": "medium",
        "tasks": list(tasks),
        "changed_files": list(changed_paths),
        "guard_violations": list(violations),
        "diagnostics": diagnostics,
        "last_message_files": last_messages,
        "written_at": now_iso(),
    }
    json_path.write_text(json.dumps(record, indent=4, ensure_ascii=False) + "\n", encoding="utf-8")
    write_batch_markdown(
        md_path,
        batch_number,
        status,
        exit_code,
        tasks,
        changed_paths,
        violations,
        diagnostics,
        last_messages,
        json_path.resolve(),
    )


def write_batch_markdown(
    path: Path,
    batch_number: int,
    status: str,
    exit_code: int,
    tasks: Sequence[str],
    changed_paths: Sequence[str],
    violations: Sequence[str],
    diagnostics: Sequence[str],
    last_messages: Sequence[str],
    json_path: Path,
) -> None:
    batch_label = f"{batch_number:02d}"
    lines: list[str] = [
        f"# Standard task list batch {batch_label}",
        "",
        f"Status: {status}",
        f"Exit code: {exit_code}",
        "Model: gpt-5.5",
        "Reasoning effort: medium",
        f"Summary JSON: {json_path}",
        "",
        "## Tasks",
    ]
    lines.extend(f"- {task}" for task in tasks)
    lines.extend(["", "## Changed files"])
    lines.extend(["- (none)"] if not changed_paths else [f"- {path}" for path in changed_paths])
    lines.extend(["", "## Guard findings"])
    lines.extend(["- PASS"] if not violations else [f"- {violation}" for violation in violations])
    lines.extend(["", "## Diagnostic summary"])
    lines.extend(f"- {diagnostic}" for diagnostic in diagnostics)
    lines.extend(["", "## Last-message files"])
    lines.extend(["- (none)"] if not last_messages else [f"- {path}" for path in last_messages])
    write_lines(path, lines)


def write_failure_record(
    output_root: Path,
    batch_number: int,
    tasks: Sequence[str],
    diagnostics: Sequence[str],
) -> None:
    output_root.mkdir(parents=True, exist_ok=True)
    jsonl_path = output_root / "failures.jsonl"
    md_path = output_root / "failures.md"
    batch_label = f"{batch_number:02d}"
    record = {
        "batch": batch_number,
        "tasks": list(tasks),
        "diagnostics": list(diagnostics),
        "written_at": now_iso(),
    }
    with jsonl_path.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(record, ensure_ascii=False, separators=(",", ":")) + "\n")

    lines: list[str] = []
    if not md_path.exists():
        lines.extend(["# Standard task list failures", ""])
    lines.extend([f"## Batch {batch_label}", "", "Tasks:"])
    lines.extend(f"- {task}" for task in tasks)
    lines.extend(["", "Diagnostics:"])
    lines.extend(f"- {diagnostic}" for diagnostic in diagnostics)
    lines.append("")
    with md_path.open("a", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")

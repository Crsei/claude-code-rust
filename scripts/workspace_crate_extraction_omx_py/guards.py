from __future__ import annotations

import re
from pathlib import Path
from typing import Sequence

from .common import git_lines, git_path, resolve_repo_path, run_capture


def rust_line_count(repo_root: Path, path: str) -> int:
    full_path = resolve_repo_path(repo_root, path)
    if not full_path.exists():
        return 0
    return len(full_path.read_text(encoding="utf-8").splitlines())


def head_rust_line_count(repo_root: Path, path: str) -> int | None:
    proc = run_capture(
        ["git", "show", f"HEAD:{git_path(path)}"],
        cwd=repo_root,
        allow_failure=True,
    )
    if proc.returncode != 0:
        return None
    return len(proc.stdout.splitlines())


def invoke_diff_guards(
    repo_root: Path,
    changed_paths: Sequence[str],
    warn_lines: int,
    max_lines: int,
    max_files: int,
    max_diff_lines: int,
    max_oversized_files_before_blocker: int,
) -> list[str]:
    findings: list[str] = []
    if not changed_paths:
        return findings

    if len(changed_paths) > max_files:
        findings.append(
            f"WARNING: batch changed {len(changed_paths)} files; consider reducing batch size for reviewability"
        )

    name_status = git_lines(
        repo_root,
        "diff",
        "--name-status",
        "HEAD",
        "--",
        *changed_paths,
        allow_failure=True,
    )
    rename_count = sum(1 for line in name_status if re.match(r"^[RC]", line))
    if rename_count > 5:
        findings.append(
            f"BLOCKER-RISK: batch has {rename_count} rename/copy entries; split mechanical moves if review becomes unclear"
        )

    findings.extend(
        file_size_findings(
            repo_root,
            changed_paths,
            warn_lines,
            max_lines,
            max_oversized_files_before_blocker,
        )
    )
    findings.extend(diff_size_findings(repo_root, changed_paths, max_diff_lines))
    return findings


def file_size_findings(
    repo_root: Path,
    changed_paths: Sequence[str],
    warn_lines: int,
    max_lines: int,
    max_oversized_files_before_blocker: int,
) -> list[str]:
    findings: list[str] = []
    oversized_candidates: list[tuple[str, int, int | None]] = []
    for path in changed_paths:
        if not path.lower().endswith(".rs"):
            continue
        if not resolve_repo_path(repo_root, path).exists():
            continue

        line_count = rust_line_count(repo_root, path)
        if line_count > max_lines:
            head_count = head_rust_line_count(repo_root, path)
            if head_count is not None and head_count > max_lines and line_count <= head_count:
                findings.append(
                    f"WARNING: {path} has {line_count} lines, above max {max_lines} "
                    f"but did not grow from HEAD ({head_count}); keep future edits split-focused"
                )
            else:
                oversized_candidates.append((path, line_count, head_count))
        elif line_count > warn_lines:
            findings.append(
                f"WARNING: {path} has {line_count} lines, above warning threshold {warn_lines}"
            )
    findings.extend(
        oversized_candidate_findings(
            oversized_candidates,
            max_lines,
            max_oversized_files_before_blocker,
        )
    )
    return findings


def oversized_candidate_findings(
    candidates: Sequence[tuple[str, int, int | None]],
    max_lines: int,
    max_oversized_files_before_blocker: int,
) -> list[str]:
    if not candidates:
        return []

    threshold = max(1, max_oversized_files_before_blocker)
    if len(candidates) >= threshold:
        sample = ", ".join(f"{path} ({line_count} lines)" for path, line_count, _ in candidates)
        return [
            f"BLOCKER-RISK: {len(candidates)} changed Rust files exceed {max_lines} lines "
            f"(risk threshold {threshold}): {sample}"
        ]

    findings: list[str] = []
    for path, line_count, head_count in candidates:
        baseline = "new file" if head_count is None else f"HEAD had {head_count} lines"
        findings.append(
            f"WARNING: {path} has {line_count} lines, above max {max_lines} "
            f"({baseline}); below blocker threshold {threshold} oversized files"
        )
    return findings


def diff_size_findings(
    repo_root: Path,
    changed_paths: Sequence[str],
    max_diff_lines: int,
) -> list[str]:
    findings: list[str] = []
    numstat = git_lines(
        repo_root,
        "diff",
        "--numstat",
        "HEAD",
        "--",
        *changed_paths,
        allow_failure=True,
    )
    for line in numstat:
        parts = line.split("\t")
        if len(parts) < 3:
            continue
        try:
            changed_total = int(parts[0]) + int(parts[1])
        except ValueError:
            continue
        if changed_total > max_diff_lines:
            findings.append(
                f"WARNING: {parts[2]} has {changed_total} changed lines; review whether the task should be split"
            )
    return findings


def agent_reported_findings(batch_output_dir: Path) -> list[str]:
    findings: list[str] = []
    if not batch_output_dir.exists():
        return findings

    for file_path in sorted(batch_output_dir.glob("task-*.last-message.txt")):
        for line_number, line in enumerate(
            file_path.read_text(encoding="utf-8").splitlines(),
            start=1,
        ):
            findings.extend(agent_line_findings(file_path.name, line_number, line))
    return findings


def agent_line_findings(file_name: str, line_number: int, line: str) -> list[str]:
    match = re.match(r"^\s*(ERROR|BLOCKER)\b\s*[:：]?\s*(.*)$", line)
    if match:
        return [
            f"{match.group(1)}: agent reported {match.group(1)} in "
            f"{file_name}:{line_number} - {match.group(2)}"
        ]

    match = re.match(r"^\s*Status\s*[:：]\s*(ERROR|BLOCKER)\b\s*(.*)$", line)
    if match:
        return [
            f"{match.group(1)}: agent status {match.group(1)} in "
            f"{file_name}:{line_number} - {match.group(2)}"
        ]

    match = re.match(r"^\s*WARNING\b\s*[:：]?\s*(.*)$", line)
    if match:
        return [
            f"WARNING: agent reported WARNING in {file_name}:{line_number} - {match.group(1)}"
        ]
    return []


def has_blocking_finding(findings: Sequence[str]) -> bool:
    return any(re.match(r"^(BLOCKER|ERROR):", finding) for finding in findings)


def has_warning_finding(findings: Sequence[str]) -> bool:
    return any(
        finding.startswith(("WARNING:", "BLOCKER-RISK:", "WAIVED-BLOCKER-RISK:"))
        for finding in findings
    )

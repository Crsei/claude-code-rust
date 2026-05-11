from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Sequence


def commit_batch(
    repo_root: Path,
    changed_paths: Sequence[str],
    batch_number: int,
    output_root: Path,
    commit_baseline_dirty_changes: bool,
) -> None:
    if not changed_paths:
        print("No new changes to commit for this batch.")
        return

    proc = subprocess.run(["git", "add", "--", *changed_paths], cwd=str(repo_root))
    if proc.returncode != 0:
        raise RuntimeError(f"git add failed for batch {batch_number}")

    diff_proc = subprocess.run(["git", "diff", "--cached", "--quiet", "--"], cwd=str(repo_root))
    if diff_proc.returncode == 0:
        print("No staged changes to commit for this batch.")
        return
    if diff_proc.returncode not in (0, 1):
        raise RuntimeError(f"git diff --cached failed for batch {batch_number}")

    batch_label = f"{batch_number:02d}"
    message_path = output_root / f"batch-{batch_label}.commit-message.txt"
    baseline_policy = (
        "Baseline dirty paths are included when their content changes during a batch"
        if commit_baseline_dirty_changes
        else "Baseline dirty paths are excluded from commit candidates"
    )
    message = f"""Advance standard task list batch {batch_label}

This checkpoint keeps the task-list lane reviewable by committing only paths
selected by the batch runner. Batch artifacts are written under {output_root} for
local inspection.

Constraint: Model fixed to gpt-5.5 with medium reasoning
Constraint: {baseline_policy}
Rejected: One large final commit | review and rollback would be harder
Confidence: medium
Scope-risk: moderate
Directive: Do not continue from a failed batch without reading the batch summary and failures log
Tested: See {output_root}/batch-{batch_label}.summary.md
Not-tested: Full workspace test unless the final verification task reports it
"""
    message_path.write_text(message, encoding="utf-8")
    proc = subprocess.run(["git", "commit", "-F", str(message_path)], cwd=str(repo_root))
    if proc.returncode != 0:
        raise RuntimeError(f"git commit failed for batch {batch_number}")

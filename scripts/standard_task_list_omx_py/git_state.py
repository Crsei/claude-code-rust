from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Iterable, Sequence

from .common import git_lines, path_key, resolve_repo_path


def git_changed_paths(repo_root: Path) -> list[str]:
    tracked = git_lines(repo_root, "diff", "--name-only", "HEAD", "--")
    untracked = git_lines(repo_root, "ls-files", "--others", "--exclude-standard")
    return sorted(set(tracked + untracked), key=str.lower)


def path_signature(repo_root: Path, path: str) -> str:
    full_path = resolve_repo_path(repo_root, path)
    if not full_path.is_file():
        return "missing"

    proc = subprocess.run(
        ["git", "hash-object", "--", path],
        cwd=str(repo_root),
        text=True,
        encoding="utf-8",
        errors="replace",
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    blob = proc.stdout.strip()
    if proc.returncode == 0 and blob:
        return f"blob:{blob}"

    stat = full_path.stat()
    return f"file:{stat.st_size}:{stat.st_mtime_ns}"


def path_signature_map(repo_root: Path, paths: Iterable[str]) -> dict[str, str]:
    return {path_key(path): path_signature(repo_root, path) for path in paths}


def batch_changed_paths(
    repo_root: Path,
    current_changed: Sequence[str],
    baseline_dirty: set[str],
    baseline_signatures: dict[str, str],
    include_baseline_dirty_changes: bool,
) -> list[str]:
    paths: list[str] = []
    for path in current_changed:
        key = path_key(path)
        if key not in baseline_dirty:
            paths.append(path)
            continue

        if not include_baseline_dirty_changes:
            continue

        old_signature = baseline_signatures.get(key, "")
        if path_signature(repo_root, path) != old_signature:
            paths.append(path)

    return sorted(dict.fromkeys(paths), key=str.lower)

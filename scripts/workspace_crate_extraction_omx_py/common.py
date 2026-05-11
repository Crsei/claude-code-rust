from __future__ import annotations

import subprocess
from datetime import datetime
from pathlib import Path
from typing import Iterable, Sequence


GLOBAL_CONTRACT = (
    "Global contract: use concise output with no long reasoning transcript; "
    "model and reasoning are fixed by the runner to gpt-5.5 medium; use native "
    "subagents only for independent bounded work; stay inside the task ownership "
    "scope; do not git commit because the runner owns commits; report "
    "PASS/WARNING/BLOCKER/ERROR; avoid redundant safety layers; make errors "
    "explicit and agent-debuggable; record effects/defects/follow-ups when the "
    "task changes code or docs; split touched Rust files over guard thresholds "
    "instead of growing large files."
)


def now_iso() -> str:
    return datetime.now().astimezone().isoformat()


def path_key(path: str) -> str:
    return path.replace("\\", "/").lower()


def git_path(path: str | Path) -> str:
    return str(path).replace("\\", "/")


def resolve_repo_path(repo_root: Path, path: str) -> Path:
    candidate = Path(path)
    if candidate.is_absolute():
        return candidate
    return repo_root / candidate


def run_capture(
    args: Sequence[str],
    *,
    cwd: Path,
    allow_failure: bool = False,
) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        list(args),
        cwd=str(cwd),
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if proc.returncode != 0 and not allow_failure:
        command = " ".join(args)
        detail = (proc.stderr or proc.stdout or "").strip()
        raise RuntimeError(f"command failed ({proc.returncode}): {command}\n{detail}")
    return proc


def git_lines(repo_root: Path, *args: str, allow_failure: bool = False) -> list[str]:
    proc = run_capture(["git", *args], cwd=repo_root, allow_failure=allow_failure)
    if proc.returncode != 0:
        return []
    return [line for line in proc.stdout.splitlines() if line]


def write_lines(path: Path, lines: Iterable[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")

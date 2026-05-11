from __future__ import annotations

import re
from pathlib import Path
from typing import Sequence


def load_task_list(path: Path) -> list[str]:
    if not path.exists():
        raise FileNotFoundError(f"Task file not found: {path}")
    tasks: list[str] = []
    for raw_line in path.read_text(encoding="utf-8-sig").splitlines():
        line = raw_line.strip()
        if line and not line.startswith("#"):
            tasks.append(line)
    return tasks


def is_checkpoint_task(task: str) -> bool:
    return re.match(r"^\[(checkpoint|review|final)\]", task) is not None


def next_batch_size(
    tasks: Sequence[str],
    index: int,
    batch_size: int,
    remaining: int,
) -> int:
    if is_checkpoint_task(tasks[index]):
        return 1

    take = min(batch_size, remaining)
    for offset in range(1, take):
        if is_checkpoint_task(tasks[index + offset]):
            return offset
    return take

from __future__ import annotations

import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

from .common import now_iso


DECISION_KEEP = "KEEP"
DECISION_WAIVE = "WAIVE"


@dataclass(frozen=True)
class BlockerReviewResult:
    exit_code: int
    decision: str
    log_path: Path

    @property
    def waived(self) -> bool:
        return self.exit_code == 0 and self.decision == DECISION_WAIVE


def blocking_findings(findings: Sequence[str]) -> list[str]:
    return [finding for finding in findings if finding.startswith("BLOCKER:")]


def blocker_risk_findings(findings: Sequence[str]) -> list[str]:
    return [finding for finding in findings if finding.startswith("BLOCKER-RISK:")]


def has_hard_error_finding(findings: Sequence[str]) -> bool:
    return any(finding.startswith("ERROR:") for finding in findings)


def should_review_blockers(exit_code: int, findings: Sequence[str]) -> bool:
    return (
        exit_code == 0
        and bool(blocker_risk_findings(findings))
        and not blocking_findings(findings)
        and not has_hard_error_finding(findings)
    )


def parse_blocker_review_decision(text: str) -> str:
    for line in text.splitlines():
        match = re.search(r"\bBLOCKER_REVIEW\s*:\s*(WAIVE|KEEP)\b", line, re.IGNORECASE)
        if match:
            return match.group(1).upper()
    return ""


def apply_blocker_review_waiver(findings: Sequence[str], review_log: Path) -> list[str]:
    waived_count = 0
    updated: list[str] = []
    for finding in findings:
        if finding.startswith("BLOCKER-RISK:"):
            waived_count += 1
            updated.append(f"WAIVED-BLOCKER-RISK: {finding.removeprefix('BLOCKER-RISK:').strip()}")
        else:
            updated.append(finding)
    updated.append(
        f"WARNING: blocker review waived {waived_count} blocker risk finding(s); see {review_log.resolve()}"
    )
    return updated


def apply_blocker_review_result(
    findings: Sequence[str],
    result: BlockerReviewResult,
) -> list[str]:
    if result.waived:
        return apply_blocker_review_waiver(findings, result.log_path)

    updated = list(findings)
    if result.exit_code != 0:
        updated.append(
            f"BLOCKER-RISK: blocker review failed with exit code {result.exit_code}; continuing with risk marker; see {result.log_path.resolve()}"
        )
    elif not result.decision:
        updated.append(
            f"BLOCKER-RISK: blocker review did not return BLOCKER_REVIEW: WAIVE or KEEP; continuing with risk marker; see {result.log_path.resolve()}"
        )
    else:
        updated.append(
            f"BLOCKER-RISK: blocker review kept risk findings for follow-up; continuing; see {result.log_path.resolve()}"
        )
    return updated


def review_blockers(
    *,
    repo_root: Path,
    batch_number: int,
    tasks: Sequence[str],
    changed_paths: Sequence[str],
    findings: Sequence[str],
    batch_output_dir: Path,
    codex: str,
    model: str,
    reasoning_effort: str,
    sandbox: str,
    timeout_seconds: int,
) -> BlockerReviewResult:
    batch_output_dir.mkdir(parents=True, exist_ok=True)
    log_path = batch_output_dir / "blocker-review.txt"
    codex_executable = shutil.which(codex) or codex
    command = [
        codex_executable,
        "exec",
        "--cd",
        str(repo_root),
        "--sandbox",
        sandbox,
        "--model",
        model,
        "--config",
        f'model_reasoning_effort="{reasoning_effort}"',
        "-",
    ]
    prompt = blocker_review_prompt(batch_number, tasks, changed_paths, findings)
    with log_path.open("w", encoding="utf-8", errors="replace") as log:
        log.write(f"Started: {now_iso()}\n")
        log.write("Command: " + " ".join(command) + "\n\n")
        log.write("## Prompt\n\n")
        log.write(prompt)
        log.write("\n\n## Output\n\n")
        try:
            proc = subprocess.run(
                command,
                cwd=str(repo_root),
                input=prompt,
                text=True,
                encoding="utf-8",
                errors="replace",
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                timeout=timeout_seconds if timeout_seconds > 0 else None,
            )
        except subprocess.TimeoutExpired as exc:
            output = exc.stdout or ""
            if isinstance(output, bytes):
                output = output.decode("utf-8", errors="replace")
            log.write(output)
            log.write(f"\nTimed out after {timeout_seconds} seconds.\n")
            return BlockerReviewResult(exit_code=124, decision="", log_path=log_path)
        except OSError as exc:
            log.write(f"ERROR: failed to start blocker review command ({codex_executable}): {exc}\n")
            return BlockerReviewResult(exit_code=1, decision="", log_path=log_path)

        output = proc.stdout or ""
        log.write(output)
        log.write(f"\nFinished: {now_iso()}\n")
        log.write(f"Exit code: {proc.returncode}\n")
    return BlockerReviewResult(
        exit_code=proc.returncode,
        decision=parse_blocker_review_decision(output),
        log_path=log_path,
    )


def blocker_review_prompt(
    batch_number: int,
    tasks: Sequence[str],
    changed_paths: Sequence[str],
    findings: Sequence[str],
) -> str:
    task_lines = "\n".join(f"- {task}" for task in tasks) or "- (none)"
    path_lines = "\n".join(f"- {path}" for path in changed_paths) or "- (none)"
    finding_lines = "\n".join(f"- {finding}" for finding in findings) or "- (none)"
    return f"""You are the read-only blocker-risk reviewer for workspace crate extraction batch {batch_number:02d}.

Inspect the current git diff and the changed files listed below. Do not edit files,
do not run git add/commit, and do not modify the worktree.

Decide whether the BLOCKER-RISK findings appear to require follow-up changes.
The runner will continue either way; your decision only controls how the risk is
marked in the batch summary. Return exactly one decision line:

BLOCKER_REVIEW: WAIVE

or:

BLOCKER_REVIEW: KEEP

Use WAIVE only when the risk is safe to ignore for this batch and no code, task
splitting, or scope reduction is needed. Use KEEP if the risk should remain
visible for follow-up.

Tasks:
{task_lines}

Changed paths:
{path_lines}

Guard findings:
{finding_lines}

After the decision line, include a concise reason and any follow-up note.
"""

from __future__ import annotations

import json
import sys
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.standard_task_list_omx_py import (
    artifacts,
    blocker_review,
    common,
    guards,
    supervisor,
    tasks,
    validation,
)


class StandardTaskListRunnerTests(unittest.TestCase):
    def test_load_task_list_ignores_comments_and_blanks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            task_file = Path(tmp) / "tasks.txt"
            task_file.write_text(
                "# comment\n\n[build] first\n  [review] second  \n",
                encoding="utf-8",
            )

            self.assertEqual(
                tasks.load_task_list(task_file),
                ["[build] first", "[review] second"],
            )

    def test_next_batch_size_stops_before_review_checkpoint(self) -> None:
        task_list = ["[build] a", "[build] b", "[review] c", "[build] d"]

        self.assertEqual(tasks.next_batch_size(task_list, 0, 3, len(task_list)), 2)
        self.assertEqual(tasks.next_batch_size(task_list, 2, 3, 2), 1)

    def test_oversized_file_blocks_only_after_threshold(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            self._git(repo, "init")
            self._git(repo, "config", "user.email", "test@example.com")
            self._git(repo, "config", "user.name", "Test")

            big_file = repo / "big.rs"
            big_file.write_text("\n".join(f"line_{i}" for i in range(6)) + "\n", encoding="utf-8")
            self._git(repo, "add", "big.rs")
            self._git(repo, "commit", "-m", "baseline")

            big_file.write_text(
                "\n".join(["changed"] + [f"line_{i}" for i in range(1, 6)]) + "\n",
                encoding="utf-8",
            )
            findings = guards.invoke_diff_guards(repo, ["big.rs"], 3, 5, 12, 600, 3)
            self.assertTrue(any("did not grow from HEAD" in finding for finding in findings))
            self.assertFalse(any(finding.startswith("BLOCKER:") for finding in findings))

            big_file.write_text("\n".join(f"line_{i}" for i in range(7)) + "\n", encoding="utf-8")
            findings = guards.invoke_diff_guards(repo, ["big.rs"], 3, 5, 12, 600, 3)
            self.assertTrue(any("below blocker threshold 3 oversized files" in finding for finding in findings))
            self.assertFalse(any(finding.startswith("BLOCKER:") for finding in findings))

            for name in ("big2.rs", "big3.rs"):
                (repo / name).write_text("\n".join(f"line_{i}" for i in range(7)) + "\n", encoding="utf-8")
            findings = guards.invoke_diff_guards(repo, ["big.rs", "big2.rs", "big3.rs"], 3, 5, 12, 600, 3)
            self.assertTrue(
                any(finding.startswith("BLOCKER-RISK: 3 changed Rust files exceed 5 lines") for finding in findings)
            )
            self.assertFalse(guards.has_blocking_finding(findings))
            self.assertEqual(artifacts.batch_status(0, findings), "WARNING")

    def test_write_batch_artifacts_records_warning_status(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output_root = Path(tmp) / "out"
            batch_dir = output_root / "batch-01"
            batch_dir.mkdir(parents=True)
            (batch_dir / "task-01.last-message.txt").write_text("done\n", encoding="utf-8")

            artifacts.write_batch_artifacts(
                output_root,
                1,
                ["[review] api-models-02"],
                0,
                ["changed.rs"],
                ["WARNING: changed.rs has 501 lines, above warning threshold 500"],
                batch_dir,
            )

            record = json.loads((output_root / "batch-01.summary.json").read_text(encoding="utf-8"))
            self.assertEqual(record["status"], "WARNING")
            self.assertEqual(record["exit_code"], 0)
            self.assertEqual(record["changed_files"], ["changed.rs"])
            self.assertIn("task-01.last-message.txt", record["last_message_files"][0])
            self.assertIn("Status: WARNING", (output_root / "batch-01.summary.md").read_text(encoding="utf-8"))

    def test_run_report_includes_task_execution_report(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            repo = Path(tmp)
            self._git(repo, "init")
            self._git(repo, "config", "user.email", "test@example.com")
            self._git(repo, "config", "user.name", "Test")
            output_root = repo / "out"
            batch_dir = output_root / "batch-01"
            batch_dir.mkdir(parents=True)
            self._write_summary(output_root / "batch-01.summary.json", "PASS", ["[build] a"])
            (batch_dir / "task-01.summary.json").write_text(
                json.dumps(
                    {
                        "task_number": 1,
                        "task_count": 1,
                        "task": "[build] a",
                        "status": "PASS",
                        "exit_code": 0,
                        "changed_files": ["src/lib.rs"],
                        "last_message_file": str(batch_dir / "task-01.last-message.txt"),
                        "output_dir": str(batch_dir),
                        "started_at": "start",
                        "ended_at": "end",
                        "interruption_reason": "",
                    }
                ),
                encoding="utf-8",
            )

            now = validation.datetime.now().astimezone()
            validation.write_run_report(repo, output_root, now, now, 1, 1, False, "", [], False)

            execution = json.loads((output_root / "execution-report.json").read_text(encoding="utf-8"))
            self.assertEqual(execution[0]["task"], "[build] a")
            self.assertEqual(execution[0]["changed_files"], ["src/lib.rs"])
            final_report = (output_root / "final-report.md").read_text(encoding="utf-8")
            self.assertIn("## Task execution report", final_report)
            self.assertIn("execution-report.md", final_report)

    def test_blocker_review_waiver_converts_blockers_to_warning_status(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            log_path = Path(tmp) / "blocker-review.txt"
            findings = ["BLOCKER-RISK: three files exceed 2000 lines", "WARNING: large diff"]
            result = blocker_review.BlockerReviewResult(
                exit_code=0,
                decision=blocker_review.DECISION_WAIVE,
                log_path=log_path,
            )

            updated = blocker_review.apply_blocker_review_result(findings, result)

            self.assertFalse(guards.has_blocking_finding(updated))
            self.assertTrue(any(finding.startswith("WAIVED-BLOCKER-RISK:") for finding in updated))
            self.assertEqual(artifacts.batch_status(0, updated), "WARNING")

    def test_blocker_review_keep_keeps_risk_non_blocking(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            log_path = Path(tmp) / "blocker-review.txt"
            findings = ["BLOCKER-RISK: three files exceed 2000 lines"]
            result = blocker_review.BlockerReviewResult(
                exit_code=0,
                decision=blocker_review.DECISION_KEEP,
                log_path=log_path,
            )

            updated = blocker_review.apply_blocker_review_result(findings, result)

            self.assertFalse(guards.has_blocking_finding(updated))
            self.assertTrue(any("kept risk findings" in finding for finding in updated))
            self.assertEqual(artifacts.batch_status(0, updated), "WARNING")

    def test_blocker_review_only_reviews_soft_risks(self) -> None:
        self.assertTrue(blocker_review.should_review_blockers(0, ["BLOCKER-RISK: large files"]))
        self.assertFalse(blocker_review.should_review_blockers(1, ["BLOCKER-RISK: large files"]))
        self.assertFalse(blocker_review.should_review_blockers(0, ["BLOCKER: agent reported blocker"]))
        self.assertFalse(
            blocker_review.should_review_blockers(
                0,
                ["BLOCKER-RISK: large files", "ERROR: agent reported error"],
            )
        )

    def test_blocker_review_requires_explicit_decision(self) -> None:
        self.assertEqual(
            blocker_review.parse_blocker_review_decision("reason\nBLOCKER_REVIEW: WAIVE\n"),
            blocker_review.DECISION_WAIVE,
        )
        self.assertEqual(
            blocker_review.parse_blocker_review_decision("BLOCKER_REVIEW: KEEP\n"),
            blocker_review.DECISION_KEEP,
        )
        self.assertEqual(blocker_review.parse_blocker_review_decision("looks fine\n"), "")

    def test_supervisor_remaining_tasks_use_summaries_and_resolution_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output_root = Path(tmp) / "out"
            output_root.mkdir()
            all_tasks = ["[build] a", "[review] b", "[build] c"]
            self._write_summary(output_root / "batch-01.summary.json", "PASS", [all_tasks[0]])
            self._write_summary(output_root / "batch-02.summary.json", "BLOCKER", [all_tasks[1]])
            (output_root / "batch-02.resolution.md").write_text("Resolved by follow-up validation.\n", encoding="utf-8")

            progress = supervisor.collect_progress([output_root])
            remaining = supervisor.remaining_tasks(all_tasks, progress.completed_keys)

            self.assertEqual(remaining, [all_tasks[2]])

    def test_supervisor_keeps_unresolved_blocker_in_remaining_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            output_root = Path(tmp) / "out"
            output_root.mkdir()
            all_tasks = ["[build] a", "[review] b"]
            self._write_summary(output_root / "batch-01.summary.json", "BLOCKER", [all_tasks[0]])

            progress = supervisor.collect_progress([output_root], trust_resolution_files=False)
            remaining = supervisor.remaining_tasks(all_tasks, progress.completed_keys)

            self.assertEqual(remaining, all_tasks)
            self.assertEqual(len(progress.unresolved_batches), 1)

    def test_supervisor_classifies_crash_without_summary_as_runner_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            attempt_output = root / "attempt"
            attempt_output.mkdir()
            log_file = root / "attempt.log"
            log_file.write_text("Traceback (most recent call last)\nNameError: boom\n", encoding="utf-8")
            before = supervisor.collect_progress([attempt_output])
            after = supervisor.collect_progress([attempt_output])

            self.assertTrue(supervisor.looks_like_runner_failure(1, attempt_output, log_file, before, after))

    def test_supervisor_does_not_classify_guard_blocker_as_runner_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            attempt_output = root / "attempt"
            attempt_output.mkdir()
            self._write_summary(attempt_output / "batch-01.summary.json", "BLOCKER", ["[build] a"])
            log_file = root / "attempt.log"
            log_file.write_text("Batch 01 did not pass. Summary: batch-01.summary.md\n", encoding="utf-8")
            before = supervisor.collect_progress([])
            after = supervisor.collect_progress([attempt_output], trust_resolution_files=False)

            self.assertFalse(supervisor.looks_like_runner_failure(1, attempt_output, log_file, before, after))

    def test_supervisor_refreshes_status_after_failed_attempt(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            task_file = root / "tasks.txt"
            task_file.write_text("[build] a\n[review] b\n", encoding="utf-8")
            fake_runner = root / "fake_runner.py"
            fake_runner.write_text(
                "\n".join(
                    [
                        "import argparse, json",
                        "from pathlib import Path",
                        "parser = argparse.ArgumentParser()",
                        "parser.add_argument('--tasks-file')",
                        "parser.add_argument('--output-root')",
                        "args, _ = parser.parse_known_args()",
                        "output = Path(args.output_root)",
                        "output.mkdir(parents=True, exist_ok=True)",
                        "task = Path(args.tasks_file).read_text(encoding='utf-8').splitlines()[0]",
                        "record = {'batch': 1, 'status': 'PASS', 'exit_code': 0, 'tasks': [task]}",
                        "(output / 'batch-01.summary.json').write_text(json.dumps(record), encoding='utf-8')",
                        "raise SystemExit(1)",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            supervisor_root = root / "supervisor"

            exit_code = supervisor.main(
                [
                    "--tasks-file",
                    str(task_file),
                    "--runner-script",
                    str(fake_runner),
                    "--supervisor-output-root",
                    str(supervisor_root),
                    "--observed-output-root",
                    str(root / "observed"),
                    "--max-attempts",
                    "1",
                    "--no-trust-resolution-files",
                ]
            )

            self.assertEqual(exit_code, 1)
            report = json.loads((supervisor_root / "supervisor-status.json").read_text(encoding="utf-8"))
            self.assertEqual(report["completed_tasks"], 1)
            self.assertEqual(report["remaining_tasks"], 1)
            remaining_file = Path(report["remaining_tasks_file"])
            self.assertEqual(remaining_file.name, "remaining-after-attempt-01.tasks.txt")
            self.assertEqual(remaining_file.read_text(encoding="utf-8").splitlines(), ["[review] b"])

    def test_run_capture_replaces_invalid_utf8_output(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            proc = common.run_capture(
                [
                    sys.executable,
                    "-c",
                    "import sys; sys.stdout.buffer.write(bytes([0x94])); sys.stderr.buffer.write(bytes([0x95]))",
                ],
                cwd=Path(tmp),
            )

            self.assertEqual(proc.returncode, 0)
            self.assertIn("\ufffd", proc.stdout)
            self.assertIn("\ufffd", proc.stderr)

    def test_repair_runner_reports_missing_codex_without_crashing(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            attempt_output = root / "attempt"
            attempt_output.mkdir()

            exit_code = supervisor.repair_runner(
                root,
                root / "runner.py",
                root / "supervisor-status.md",
                root / "attempt.log",
                attempt_output,
                "definitely-missing-codex-executable",
                "gpt-5.5",
                "medium",
                1,
            )

            self.assertEqual(exit_code, 1)
            repair_logs = list(attempt_output.glob("supervisor-repair-*.log"))
            self.assertEqual(len(repair_logs), 1)
            self.assertIn("failed to start codex repair command", repair_logs[0].read_text(encoding="utf-8"))

    def _write_summary(self, path: Path, status: str, task_list: list[str]) -> None:
        path.write_text(
            json.dumps(
                {
                    "batch": 1,
                    "status": status,
                    "exit_code": 0,
                    "tasks": task_list,
                    "changed_files": [],
                    "guard_violations": [],
                    "diagnostics": [],
                },
                ensure_ascii=False,
            ),
            encoding="utf-8",
        )

    def _git(self, repo: Path, *args: str) -> None:
        subprocess.run(
            ["git", *args],
            cwd=str(repo),
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )


if __name__ == "__main__":
    unittest.main()

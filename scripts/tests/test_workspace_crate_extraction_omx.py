from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path

from scripts.workspace_crate_extraction_omx_py import artifacts, guards, tasks


class WorkspaceCrateExtractionRunnerTests(unittest.TestCase):
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

    def test_historical_oversized_file_warns_unless_it_grows(self) -> None:
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
            findings = guards.invoke_diff_guards(repo, ["big.rs"], 3, 5, 12, 600)
            self.assertTrue(any("did not grow from HEAD" in finding for finding in findings))
            self.assertFalse(any(finding.startswith("BLOCKER:") for finding in findings))

            big_file.write_text("\n".join(f"line_{i}" for i in range(7)) + "\n", encoding="utf-8")
            findings = guards.invoke_diff_guards(repo, ["big.rs"], 3, 5, 12, 600)
            self.assertTrue(any(finding.startswith("BLOCKER: big.rs grew") for finding in findings))

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

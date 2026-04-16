"""Process executor — spawns ``claude-code-rs`` and reads JSONL from stdout.

Mirrors TypeScript SDK's exec.ts.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import threading
from collections import deque
from typing import Iterator

from .config import ClientOptions, SessionOptions
from .errors import BinaryNotFoundError, ParseError, ProcessError

INTERNAL_ORIGINATOR_ENV = "CC_RUST_INTERNAL_ORIGINATOR"
SDK_ORIGINATOR = "cc_rust_sdk_python"

# ---------------------------------------------------------------------------
# Binary resolution
# ---------------------------------------------------------------------------


def find_claude_code_rs_path() -> str:
    """Locate the ``claude-code-rs`` binary.

    Resolution order (mirrors TypeScript SDK exec.ts):
    1. ``CLAUDE_CODE_RS_PATH`` environment variable
    2. System PATH via ``shutil.which``
    3. Relative cargo build output (release then debug)
    """
    searched: list[str] = []

    # 1. Explicit env var
    env_path = os.environ.get("CLAUDE_CODE_RS_PATH")
    if env_path:
        return env_path

    # 2. System PATH
    which_result = shutil.which("claude-code-rs")
    if which_result:
        return which_result
    searched.append("PATH (claude-code-rs)")

    # 3. Relative to this package (cargo build output)
    binary_name = (
        "claude-code-rs.exe" if sys.platform == "win32" else "claude-code-rs"
    )
    # sdk/python/src/claude_code_rs/ → ../../.. → sdk/ → .. → project root
    package_dir = os.path.dirname(os.path.abspath(__file__))
    project_root = os.path.normpath(
        os.path.join(package_dir, "..", "..", "..", "..")
    )
    for profile in ("release", "debug"):
        candidate = os.path.join(project_root, "target", profile, binary_name)
        if os.path.isfile(candidate):
            return candidate
        searched.append(candidate)

    raise BinaryNotFoundError(searched)


# ---------------------------------------------------------------------------
# Executor
# ---------------------------------------------------------------------------


class ClaudeCodeExec:
    """Manages spawning the ``claude-code-rs`` subprocess and reading JSONL."""

    def __init__(
        self,
        executable_path: str | None = None,
        env: dict[str, str] | None = None,
    ) -> None:
        self._executable_path = executable_path or find_claude_code_rs_path()
        self._env_override = env

    def run(
        self,
        *,
        input: str,
        api_key: str | None = None,
        model: str | None = None,
        working_directory: str | None = None,
        permission_mode: str | None = None,
        max_turns: int | None = None,
        max_budget: float | None = None,
        system_prompt: str | None = None,
        append_system_prompt: str | None = None,
        verbose: bool = False,
        continue_session: str | None = None,
    ) -> Iterator[str]:
        """Spawn the CLI, write *input* to stdin, yield stdout lines.

        This is a generator function. The subprocess is cleaned up in the
        ``finally`` block when the generator is closed or exhausted.
        """
        args = self._build_args(
            model=model,
            working_directory=working_directory,
            permission_mode=permission_mode,
            max_turns=max_turns,
            max_budget=max_budget,
            system_prompt=system_prompt,
            append_system_prompt=append_system_prompt,
            verbose=verbose,
            continue_session=continue_session,
        )
        env = self._build_env(api_key=api_key)

        proc = subprocess.Popen(
            args,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
        )

        # Collect stderr in a background daemon thread to avoid deadlock
        stderr_lines: deque[str] = deque(maxlen=400)
        stderr_thread = threading.Thread(
            target=_drain_stderr, args=(proc, stderr_lines), daemon=True
        )
        stderr_thread.start()

        try:
            # Write prompt and close stdin
            assert proc.stdin is not None
            proc.stdin.write(input.encode("utf-8"))
            proc.stdin.close()

            # Yield stdout lines
            assert proc.stdout is not None
            for raw_line in proc.stdout:
                line = raw_line.decode("utf-8", errors="replace").rstrip("\n")
                if line:
                    yield line

            # Wait for process to finish
            exit_code = proc.wait()
            stderr_thread.join(timeout=1.0)

            if exit_code != 0:
                raise ProcessError(exit_code, "\n".join(stderr_lines))

        finally:
            # Guarantee cleanup
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=2.0)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
            stderr_thread.join(timeout=0.5)

    # -------------------------------------------------------------------
    # Internal helpers
    # -------------------------------------------------------------------

    def _build_args(
        self,
        *,
        model: str | None,
        working_directory: str | None,
        permission_mode: str | None,
        max_turns: int | None,
        max_budget: float | None,
        system_prompt: str | None,
        append_system_prompt: str | None,
        verbose: bool,
        continue_session: str | None,
    ) -> list[str]:
        args = [self._executable_path, "--output-format", "json", "-p"]

        if model:
            args += ["--model", model]
        if working_directory:
            args += ["--cwd", working_directory]
        if permission_mode:
            args += ["--permission-mode", permission_mode]
        if max_turns is not None:
            args += ["--max-turns", str(max_turns)]
        if max_budget is not None:
            args += ["--max-budget", str(max_budget)]
        if system_prompt:
            args += ["--system-prompt", system_prompt]
        if append_system_prompt:
            args += ["--append-system-prompt", append_system_prompt]
        if verbose:
            args.append("--verbose")
        if continue_session:
            args += ["--continue", continue_session]

        return args

    def _build_env(self, *, api_key: str | None) -> dict[str, str]:
        if self._env_override is not None:
            env = dict(self._env_override)
        else:
            env = dict(os.environ)

        if INTERNAL_ORIGINATOR_ENV not in env:
            env[INTERNAL_ORIGINATOR_ENV] = SDK_ORIGINATOR
        if api_key:
            env["ANTHROPIC_API_KEY"] = api_key

        return env


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _drain_stderr(
    proc: subprocess.Popen[bytes], buf: deque[str]
) -> None:
    """Background thread: read stderr lines into *buf*."""
    assert proc.stderr is not None
    for raw_line in proc.stderr:
        buf.append(raw_line.decode("utf-8", errors="replace").rstrip("\n"))

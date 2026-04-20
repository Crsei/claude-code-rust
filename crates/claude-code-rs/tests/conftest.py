"""
Pytest conftest for cc-rust E2E tests.

Cross-platform PTY/process spawning using:
- Linux/macOS: pexpect.spawn (real PTY)
- Windows: pexpect.popen_spawn.PopenSpawn (pipe-based, no PTY)

For TUI testing on Windows, use wexpect (ConPTY) — but it has reliability issues.
For headless/IPC testing, PopenSpawn works perfectly on all platforms.
"""
import os
import re
import sys
import json
import time
import platform
import subprocess
from pathlib import Path
from dataclasses import dataclass, field
from typing import Optional

import pytest

# ---------------------------------------------------------------------------
# Platform detection
# ---------------------------------------------------------------------------
IS_WINDOWS = platform.system() == "Windows"
PROJECT_ROOT = Path(__file__).parent.parent.resolve()
CARGO_TARGET = PROJECT_ROOT / "target" / "release"

# ---------------------------------------------------------------------------
# ANSI helpers
# ---------------------------------------------------------------------------
_ANSI_RE = re.compile(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\].*?\x07|\x1b\[.*?[@-~]")


def strip_ansi(text: str) -> str:
    """Remove all ANSI escape sequences from text."""
    return _ANSI_RE.sub("", text)


def has_ansi(text: str) -> bool:
    """Check if text contains any ANSI escape sequences."""
    return "\x1b[" in text


# ---------------------------------------------------------------------------
# Binary path resolution
# ---------------------------------------------------------------------------
def find_binary() -> str:
    """Find the cc-rust binary. Prefers release build."""
    if IS_WINDOWS:
        candidates = [
            CARGO_TARGET / "claude-code-rs.exe",
            CARGO_TARGET / "cc-rust.exe",
            PROJECT_ROOT / "target" / "debug" / "claude-code-rs.exe",
            PROJECT_ROOT / "target" / "debug" / "cc-rust.exe",
        ]
    else:
        candidates = [
            CARGO_TARGET / "claude-code-rs",
            CARGO_TARGET / "cc-rust",
            PROJECT_ROOT / "target" / "debug" / "claude-code-rs",
            PROJECT_ROOT / "target" / "debug" / "cc-rust",
        ]

    for p in candidates:
        if p.exists():
            return str(p)

    pytest.skip("cc-rust binary not found. Run `cargo build --release` first.")


# ---------------------------------------------------------------------------
# Spawn wrapper — cross-platform
# ---------------------------------------------------------------------------
@dataclass
class SpawnResult:
    """Wrapper around pexpect child for cross-platform compatibility."""
    child: object
    is_popen: bool = False

    def expect(self, pattern, timeout=30):
        """Wait for pattern in output. Returns index if list, 0 if string."""
        return self.child.expect(pattern, timeout=timeout)

    def expect_exact(self, pattern, timeout=30):
        """Wait for exact string (no regex)."""
        return self.child.expect_exact(pattern, timeout=timeout)

    def sendline(self, text=""):
        """Send text followed by newline."""
        return self.child.sendline(text)

    def send(self, text):
        """Send raw text (no newline)."""
        return self.child.send(text)

    @property
    def before(self) -> str:
        """Text before last match."""
        return self.child.before or ""

    @property
    def after(self) -> str:
        """Text after last match."""
        return self.child.after or ""

    @property
    def match(self):
        """The match object from last expect."""
        return self.child.match

    def read_all_available(self, timeout=1) -> str:
        """Read all currently available output."""
        import pexpect as _pexpect
        chunks = []
        while True:
            try:
                data = self.child.read_nonblocking(4096, timeout=timeout)
                if data:
                    chunks.append(data if isinstance(data, str) else data.decode("utf-8", errors="replace"))
                else:
                    break
            except (_pexpect.TIMEOUT, _pexpect.EOF):
                break
        return "".join(chunks)

    def close(self):
        """Terminate the child process."""
        try:
            if self.is_popen:
                self.child.proc.terminate()
            else:
                self.child.terminate(force=True)
        except Exception:
            pass


def spawn_process(cmd: str, timeout: int = 60, encoding: str = "utf-8",
                  env: Optional[dict] = None) -> SpawnResult:
    """
    Spawn a process for E2E testing.

    On Linux/macOS: uses pexpect.spawn (real PTY).
    On Windows: uses pexpect.popen_spawn.PopenSpawn (pipe-based).

    For cc-rust, prefer `--headless` mode which works perfectly with PopenSpawn.
    """
    import pexpect

    spawn_env = os.environ.copy()
    if env:
        spawn_env.update(env)

    if IS_WINDOWS:
        from pexpect.popen_spawn import PopenSpawn
        child = PopenSpawn(cmd, encoding=encoding, timeout=timeout, env=spawn_env)
        return SpawnResult(child=child, is_popen=True)
    else:
        child = pexpect.spawn(cmd, encoding=encoding, timeout=timeout, env=spawn_env)
        # On real PTY, set window size to avoid line wrapping issues
        child.setwinsize(50, 200)
        return SpawnResult(child=child, is_popen=False)


# ---------------------------------------------------------------------------
# Headless IPC helpers (for --headless mode)
# ---------------------------------------------------------------------------
class HeadlessClient:
    """
    Client for cc-rust's --headless JSONL IPC protocol.

    This is the recommended way to E2E test cc-rust:
    - No PTY/terminal issues
    - No ANSI escape handling needed
    - Structured JSON messages
    - Works identically on Windows and Linux
    """

    def __init__(self, binary_path: str, extra_args: list[str] = None,
                 timeout: int = 60, env: dict = None):
        self.timeout = timeout
        spawn_env = os.environ.copy()
        if env:
            spawn_env.update(env)

        cmd = [binary_path, "--headless"] + (extra_args or [])
        self.proc = subprocess.Popen(
            cmd,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=spawn_env,
        )
        self._buffer = ""

    def send_message(self, msg: dict):
        """Send a JSONL message to the headless backend."""
        line = json.dumps(msg) + "\n"
        self.proc.stdin.write(line.encode("utf-8"))
        self.proc.stdin.flush()

    def submit_prompt(self, text: str, request_id: str = "test-1"):
        """Send a user prompt."""
        self.send_message({
            "type": "submit_prompt",
            "id": request_id,
            "text": text,
        })

    def send_permission_response(self, tool_use_id: str, allow: bool):
        """Respond to a permission request."""
        self.send_message({
            "type": "permission_response",
            "tool_use_id": tool_use_id,
            "decision": "allow" if allow else "deny",
        })

    def read_message(self, timeout: float = None) -> Optional[dict]:
        """Read one JSONL message from stdout. Returns None on timeout/EOF."""
        import select as _select
        timeout = timeout or self.timeout
        deadline = time.time() + timeout

        while time.time() < deadline:
            # Check if there's a complete line in buffer
            if "\n" in self._buffer:
                line, self._buffer = self._buffer.split("\n", 1)
                line = line.strip()
                if line:
                    return json.loads(line)

            # Read more data
            remaining = max(0.1, deadline - time.time())
            try:
                data = self.proc.stdout.read1(4096) if hasattr(self.proc.stdout, 'read1') else None
                if data is None:
                    # Fallback: use readline with a thread
                    import threading
                    result = [None]
                    def _read():
                        result[0] = self.proc.stdout.readline()
                    t = threading.Thread(target=_read, daemon=True)
                    t.start()
                    t.join(timeout=remaining)
                    if result[0]:
                        self._buffer += result[0].decode("utf-8", errors="replace")
                    else:
                        return None
                else:
                    self._buffer += data.decode("utf-8", errors="replace")
            except Exception:
                return None

        return None

    def read_until(self, msg_type: str, timeout: float = None) -> list[dict]:
        """Read messages until we get one of the specified type. Returns all messages read."""
        messages = []
        while True:
            msg = self.read_message(timeout=timeout)
            if msg is None:
                break
            messages.append(msg)
            if msg.get("type") == msg_type:
                break
        return messages

    def close(self):
        """Terminate the headless backend."""
        try:
            self.send_message({"type": "quit"})
        except Exception:
            pass
        try:
            self.proc.terminate()
            self.proc.wait(timeout=5)
        except Exception:
            self.proc.kill()


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------
@pytest.fixture
def binary_path():
    """Path to the cc-rust binary."""
    return find_binary()


@pytest.fixture
def spawn(binary_path):
    """Spawn cc-rust with custom args. Returns a factory function."""
    children = []

    def _spawn(extra_args: str = "", timeout: int = 60, env: dict = None) -> SpawnResult:
        cmd = f"{binary_path} {extra_args}"
        child = spawn_process(cmd, timeout=timeout, env=env)
        children.append(child)
        return child

    yield _spawn

    for c in children:
        c.close()


@pytest.fixture
def headless(binary_path):
    """Spawn cc-rust in headless mode. Returns a factory function."""
    clients = []

    def _headless(extra_args: list[str] = None, timeout: int = 60,
                  env: dict = None) -> HeadlessClient:
        client = HeadlessClient(binary_path, extra_args=extra_args,
                                timeout=timeout, env=env)
        clients.append(client)
        return client

    yield _headless

    for c in clients:
        c.close()


# ---------------------------------------------------------------------------
# Markers
# ---------------------------------------------------------------------------
def pytest_configure(config):
    config.addinivalue_line("markers", "slow: marks tests as slow (> 30s)")
    config.addinivalue_line("markers", "requires_api: requires API key")
    config.addinivalue_line("markers", "windows_only: Windows-specific tests")
    config.addinivalue_line("markers", "unix_only: Unix-specific tests")

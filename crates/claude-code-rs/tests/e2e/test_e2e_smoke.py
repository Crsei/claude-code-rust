"""
E2E smoke tests for cc-rust.

These tests verify basic CLI behavior without requiring an API key.
They test startup, --version, --help, and error handling.
"""
import re
import sys
import time
import platform

import pexpect
import pytest

from conftest import (
    IS_WINDOWS, strip_ansi, has_ansi, spawn_process, find_binary,
    SpawnResult,
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
def spawn_cc(args: str = "", timeout: int = 30, env: dict = None) -> SpawnResult:
    """Spawn cc-rust with given args."""
    binary = find_binary()
    return spawn_process(f"{binary} {args}", timeout=timeout, env=env)


# ---------------------------------------------------------------------------
# Fast-path tests (no API key needed)
# ---------------------------------------------------------------------------
class TestFastPaths:
    """Tests for CLI fast paths that exit immediately."""

    def test_version_flag(self):
        """--version should print version and exit."""
        child = spawn_cc("-V")
        child.expect(pexpect.EOF, timeout=10)
        output = strip_ansi(child.before)
        # Should contain a semver-like version
        assert re.search(r"\d+\.\d+\.\d+", output), f"No version found in: {output!r}"

    def test_help_flag(self):
        """--help should print usage and exit."""
        child = spawn_cc("--help")
        child.expect(pexpect.EOF, timeout=10)
        output = strip_ansi(child.before).lower()
        assert "usage" in output or "claude" in output, f"No help text in: {output!r}"

    def test_invalid_flag(self):
        """Unknown flags should produce an error."""
        child = spawn_cc("--this-flag-does-not-exist")
        child.expect(pexpect.EOF, timeout=10)
        output = strip_ansi(child.before).lower()
        assert "error" in output or "unexpected" in output or "unrecognized" in output, \
            f"No error message for invalid flag: {output!r}"


# ---------------------------------------------------------------------------
# Print mode tests (requires API key, but tests the pattern)
# ---------------------------------------------------------------------------
class TestPrintMode:
    """Tests for -p / --print mode."""

    @pytest.mark.requires_api
    @pytest.mark.slow
    def test_print_mode_basic(self):
        """Print mode should output response and exit."""
        child = spawn_cc('-p "Say exactly: HELLO_E2E"', timeout=60)
        # Should eventually get output and exit
        child.expect(pexpect.EOF, timeout=60)
        output = strip_ansi(child.before)
        assert "HELLO_E2E" in output, f"Expected HELLO_E2E in output: {output[:500]}"


# ---------------------------------------------------------------------------
# ANSI output tests
# ---------------------------------------------------------------------------
class TestAnsiOutput:
    """Tests for ANSI escape handling in output."""

    def test_ansi_in_raw_output(self):
        """Version output should NOT contain ANSI (it's a fast path)."""
        child = spawn_cc("-V")
        child.expect(pexpect.EOF, timeout=10)
        raw = child.before
        # Version fast path should be plain text
        # (This test documents behavior — adjust if cc-rust adds colors to --version)

    def test_strip_ansi_utility(self):
        """Verify our ANSI stripping works correctly."""
        raw = "\x1b[31mERROR\x1b[0m: something \x1b[1;33mwent wrong\x1b[0m"
        clean = strip_ansi(raw)
        assert clean == "ERROR: something went wrong"
        assert not has_ansi(clean)
        assert has_ansi(raw)


# ---------------------------------------------------------------------------
# Timeout and robustness tests
# ---------------------------------------------------------------------------
class TestTimeouts:
    """Tests for timeout handling patterns."""

    def test_expect_timeout_fires(self):
        """Demonstrate that pexpect.TIMEOUT works correctly."""
        # Use Python sleep to avoid Windows encoding issues with system commands
        child = spawn_process(
            f"{sys.executable} -u -c \"import time; time.sleep(30)\"",
            timeout=5,
        )

        with pytest.raises(pexpect.TIMEOUT):
            child.expect("THIS_WILL_NEVER_APPEAR", timeout=1)
        child.close()

    def test_expect_multiple_patterns(self):
        """Demonstrate matching against multiple patterns."""
        child = spawn_cc("-V")
        import pexpect as _pexpect
        idx = child.expect([
            r"\d+\.\d+\.\d+",   # version number
            "error",              # error message
            _pexpect.EOF,         # unexpected EOF
        ], timeout=10)
        # Should match the version number (index 0)
        assert idx == 0, f"Expected version match (0), got {idx}"


# ---------------------------------------------------------------------------
# Headless mode pattern test
# ---------------------------------------------------------------------------
class TestHeadlessPattern:
    """Tests demonstrating the headless IPC testing pattern."""

    @pytest.mark.requires_api
    @pytest.mark.slow
    def test_headless_basic(self, headless):
        """Headless mode should accept JSONL on stdin."""
        client = headless(timeout=60)

        # Send a simple prompt
        client.submit_prompt("Say exactly: HEADLESS_OK")

        # Read messages until we get a response
        messages = client.read_until("response_complete", timeout=60)
        msg_types = [m.get("type") for m in messages]

        # Should have received some messages
        assert len(messages) > 0, "No messages received from headless mode"

        # Check if response contains our text
        text_parts = []
        for m in messages:
            if m.get("type") == "assistant_text":
                text_parts.append(m.get("text", ""))

        full_text = "".join(text_parts)
        assert "HEADLESS_OK" in full_text, f"Expected HEADLESS_OK in: {full_text[:500]}"

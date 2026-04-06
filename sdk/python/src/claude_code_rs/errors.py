"""Exception hierarchy for the claude-code-rs Python SDK."""

from __future__ import annotations


class CcRustError(Exception):
    """Base exception for the claude-code-rs SDK."""


class BinaryNotFoundError(CcRustError):
    """The claude-code-rs binary could not be located."""

    def __init__(self, searched: list[str]) -> None:
        self.searched = searched
        super().__init__(
            "claude-code-rs not found. "
            f"Searched: {', '.join(searched)}"
        )


class ProcessError(CcRustError):
    """The claude-code-rs subprocess exited with a non-zero status."""

    def __init__(self, exit_code: int, stderr: str) -> None:
        self.exit_code = exit_code
        self.stderr = stderr
        super().__init__(
            f"claude-code-rs exited with code {exit_code}: "
            f"{stderr[:500]}"
        )


class TurnExecutionError(CcRustError):
    """A turn completed with an error (SdkResult.is_error == true)."""

    def __init__(
        self,
        subtype: str,
        message: str,
        errors: list[str] | None = None,
    ) -> None:
        self.subtype = subtype
        self.errors = errors or []
        super().__init__(f"[{subtype}] {message}")


class ParseError(CcRustError):
    """Failed to parse a JSONL line from stdout."""

    def __init__(self, line: str, cause: Exception) -> None:
        self.line = line
        self.cause = cause
        super().__init__(f"Failed to parse JSONL: {cause}")

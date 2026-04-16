"""Top-level client — analogous to ``Codex`` in the Codex SDK.

Mirrors TypeScript SDK's claudeCode.ts.

Usage::

    from claude_code_rs import ClaudeCode, SessionOptions

    client = ClaudeCode()
    session = client.start_session(SessionOptions(model="claude-sonnet-4-20250514"))
    turn = session.run("What files are in this directory?")
    print(turn.final_response)
"""

from __future__ import annotations

from dataclasses import replace

from .config import ClientOptions, SessionOptions
from .exec import ClaudeCodeExec
from .session import Session


class ClaudeCode:
    """Entry point for the claude-code-rs Python SDK."""

    def __init__(self, options: ClientOptions | None = None) -> None:
        self._options = options or ClientOptions()
        self._exec = ClaudeCodeExec(
            executable_path=self._options.executable_path,
            env=self._options.env,
        )

    def start_session(
        self, options: SessionOptions | None = None
    ) -> Session:
        """Create a new session."""
        return Session(
            self._exec,
            self._options,
            options or SessionOptions(),
        )

    def resume_session(
        self,
        session_id: str,
        options: SessionOptions | None = None,
    ) -> Session:
        """Resume a previously persisted session by ID."""
        opts = options or SessionOptions()
        merged = replace(opts, continue_session=session_id)
        return Session(self._exec, self._options, merged)

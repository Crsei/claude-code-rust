"""Configuration types for the claude-code-rs Python SDK.

Mirrors TypeScript SDK's claudeCodeOptions.ts + sessionOptions.ts.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

PermissionMode = Literal["default", "auto", "bypass", "plan"]


@dataclass
class ClientOptions:
    """Options for the top-level ``ClaudeCode`` client.

    Mirrors TypeScript ``ClaudeCodeOptions``.
    """

    executable_path: str | None = None
    """Path to the ``claude-code-rs`` binary. Auto-detected if omitted."""

    api_key: str | None = None
    """API key (passed as ``ANTHROPIC_API_KEY`` env var to the subprocess)."""

    env: dict[str, str] | None = None
    """Environment variables passed to the CLI process."""


@dataclass
class SessionOptions:
    """Options for creating or resuming a session.

    Maps to ``claude-code-rs`` CLI arguments.
    Mirrors TypeScript ``SessionOptions``.
    """

    model: str | None = None
    """Model override (``--model``)."""

    working_directory: str | None = None
    """Working directory (``--cwd``)."""

    permission_mode: PermissionMode | None = None
    """Permission mode (``--permission-mode``)."""

    max_turns: int | None = None
    """Maximum number of turns for agentic loops (``--max-turns``)."""

    max_budget: float | None = None
    """Maximum budget in USD (``--max-budget``)."""

    system_prompt: str | None = None
    """Custom system prompt — replaces default (``--system-prompt``)."""

    append_system_prompt: str | None = None
    """Append to the system prompt (``--append-system-prompt``)."""

    verbose: bool = False
    """Enable verbose output (``--verbose``)."""

    continue_session: str | None = None
    """Resume a specific session by ID (``--continue``)."""

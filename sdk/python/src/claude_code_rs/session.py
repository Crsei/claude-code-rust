"""Session — represents a conversation with the ``claude-code-rs`` agent.

Analogous to ``Thread`` in the Codex SDK.
Mirrors TypeScript SDK's session.ts.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from typing import Iterator

from .config import ClientOptions, SessionOptions
from .errors import ParseError, TurnExecutionError
from .events import (
    ItemCompletedEvent,
    SessionEvent,
    SessionStartedEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    Usage,
)
from .exec import ClaudeCodeExec
from .items import AgentMessageItem, SessionItem
from .transform import transform_raw_event

# ---------------------------------------------------------------------------
# Result types
# ---------------------------------------------------------------------------


@dataclass
class Turn:
    """Buffered result of a single turn."""

    items: list[SessionItem] = field(default_factory=list)
    final_response: str = ""
    usage: Usage | None = None


@dataclass
class StreamedTurn:
    """Streaming result — wraps the event iterator."""

    events: Iterator[SessionEvent] = field(
        default_factory=lambda: iter([])
    )


# ---------------------------------------------------------------------------
# Session
# ---------------------------------------------------------------------------


class Session:
    """A conversation session with ``claude-code-rs``.

    Usage::

        session = claude.start_session(SessionOptions(model="..."))
        turn = session.run("What files are in this directory?")
        print(turn.final_response)
    """

    def __init__(
        self,
        exec: ClaudeCodeExec,
        client_options: ClientOptions,
        session_options: SessionOptions,
    ) -> None:
        self._exec = exec
        self._client_options = client_options
        self._session_options = session_options
        self._session_id: str | None = None

    @property
    def session_id(self) -> str | None:
        """Session ID — populated after the first ``system_init`` event."""
        return self._session_id

    # -------------------------------------------------------------------
    # Public API
    # -------------------------------------------------------------------

    def run(self, input: str) -> Turn:
        """Execute a turn and return a buffered result.

        Consumes the entire event stream, collects items, and returns a
        :class:`Turn`. Raises :class:`TurnExecutionError` on failure.
        """
        streamed = self.run_streamed(input)

        items: list[SessionItem] = []
        final_response = ""
        usage: Usage | None = None
        turn_failure: TurnFailedEvent | None = None

        for event in streamed.events:
            if isinstance(event, ItemCompletedEvent) and event.item is not None:
                items.append(event.item)
                if isinstance(event.item, AgentMessageItem):
                    final_response = event.item.text

            elif isinstance(event, TurnCompletedEvent):
                usage = event.usage

            elif isinstance(event, TurnFailedEvent):
                turn_failure = event

        if turn_failure and turn_failure.error:
            raise TurnExecutionError(
                subtype=turn_failure.error.subtype,
                message=turn_failure.error.message,
            )

        return Turn(items=items, final_response=final_response, usage=usage)

    def run_streamed(self, input: str) -> StreamedTurn:
        """Execute a turn and stream events as an iterator."""
        return StreamedTurn(events=self._run_streamed_internal(input))

    # -------------------------------------------------------------------
    # Internal
    # -------------------------------------------------------------------

    def _run_streamed_internal(self, input: str) -> Iterator[SessionEvent]:
        """Parse JSONL lines, transform, and yield ``SessionEvent`` instances."""
        generator = self._exec.run(
            input=input,
            api_key=self._client_options.api_key,
            model=self._session_options.model,
            working_directory=self._session_options.working_directory,
            permission_mode=self._session_options.permission_mode,
            max_turns=self._session_options.max_turns,
            max_budget=self._session_options.max_budget,
            system_prompt=self._session_options.system_prompt,
            append_system_prompt=self._session_options.append_system_prompt,
            verbose=self._session_options.verbose,
            continue_session=self._session_options.continue_session,
        )

        try:
            for line in generator:
                try:
                    raw = json.loads(line)
                except json.JSONDecodeError as exc:
                    raise ParseError(line, exc) from exc

                events = transform_raw_event(raw)
                for event in events:
                    # Capture session ID from the first system_init event
                    if (
                        isinstance(event, SessionStartedEvent)
                        and self._session_id is None
                    ):
                        self._session_id = event.session_id
                    yield event
        finally:
            # Generator cleanup is handled by exec.py's finally block
            pass

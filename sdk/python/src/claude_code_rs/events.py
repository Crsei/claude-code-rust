"""Normalized SDK event types.

These are the consumer-facing events yielded by ``Session.run_streamed()``.
Raw Rust JSONL events are transformed into these types by ``transform.py``.

Mirrors TypeScript SDK's events.ts.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Union

from .items import SessionItem

# ---------------------------------------------------------------------------
# Usage
# ---------------------------------------------------------------------------


@dataclass
class Usage:
    input_tokens: int = 0
    cached_input_tokens: int = 0
    output_tokens: int = 0
    total_cost_usd: float = 0.0


# ---------------------------------------------------------------------------
# Session lifecycle events
# ---------------------------------------------------------------------------


@dataclass
class SessionStartedEvent:
    type: str = "session.started"
    session_id: str = ""
    model: str = ""
    tools: list[str] = field(default_factory=list)
    permission_mode: str = ""


@dataclass
class TurnStartedEvent:
    type: str = "turn.started"


@dataclass
class TurnCompletedEvent:
    type: str = "turn.completed"
    usage: Usage | None = None
    result: str = ""
    num_turns: int = 0
    duration_ms: int = 0


@dataclass
class SessionError:
    message: str = ""
    subtype: str = ""


@dataclass
class TurnFailedEvent:
    type: str = "turn.failed"
    error: SessionError | None = None


# ---------------------------------------------------------------------------
# Item events
# ---------------------------------------------------------------------------


@dataclass
class ItemStartedEvent:
    type: str = "item.started"
    item: SessionItem | None = None


@dataclass
class ItemUpdatedEvent:
    type: str = "item.updated"
    item: SessionItem | None = None


@dataclass
class ItemCompletedEvent:
    type: str = "item.completed"
    item: SessionItem | None = None


# ---------------------------------------------------------------------------
# Stream delta (raw pass-through of streaming content)
# ---------------------------------------------------------------------------


@dataclass
class StreamDeltaEvent:
    type: str = "stream.delta"
    event_type: str = ""
    index: int | None = None
    delta: Any = None
    usage: Any = None
    content_block: Any = None


# ---------------------------------------------------------------------------
# Error event (retryable API errors)
# ---------------------------------------------------------------------------


@dataclass
class SessionErrorEvent:
    type: str = "error"
    message: str = ""
    retryable: bool = False
    attempt: int | None = None
    max_retries: int | None = None
    retry_delay_ms: int | None = None


# ---------------------------------------------------------------------------
# Top-level union
# ---------------------------------------------------------------------------

SessionEvent = Union[
    SessionStartedEvent,
    TurnStartedEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    ItemStartedEvent,
    ItemUpdatedEvent,
    ItemCompletedEvent,
    StreamDeltaEvent,
    SessionErrorEvent,
]

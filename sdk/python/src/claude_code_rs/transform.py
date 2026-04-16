"""Transforms raw JSONL from ``claude-code-rs --output-format json`` into
normalized ``SessionEvent`` types.

The raw JSONL matches the Rust ``SdkMessage`` serde serialization
(internally tagged with ``#[serde(tag = "type", rename_all = "snake_case")]``).

Mirrors TypeScript SDK's transform.ts.
"""

from __future__ import annotations

from typing import Any

from .events import (
    ItemCompletedEvent,
    SessionErrorEvent,
    SessionEvent,
    SessionStartedEvent,
    StreamDeltaEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    SessionError,
    Usage,
)
from .items import (
    AgentMessageItem,
    CompactBoundaryItem,
    ContentBlock,
    ImageBlock,
    ImageSource,
    MessageUsage,
    RedactedThinkingBlock,
    TextBlock,
    ThinkingBlock,
    ToolResultBlock,
    ToolUseSummaryItem,
    ToolUseBlock,
    UserReplayItem,
)


def transform_raw_event(raw: dict[str, Any]) -> list[SessionEvent]:
    """Transform a raw JSONL dict into normalized ``SessionEvent`` instances.

    Returns a list (usually with one element). Unknown message types
    produce an empty list.
    """
    match raw.get("type"):
        case "system_init":
            return [
                SessionStartedEvent(
                    session_id=raw["session_id"],
                    model=raw["model"],
                    tools=raw["tools"],
                    permission_mode=raw["permission_mode"],
                )
            ]

        case "assistant":
            return _transform_assistant(raw)

        case "user_replay":
            return [
                ItemCompletedEvent(
                    item=UserReplayItem(
                        id=raw["uuid"],
                        content=raw["content"],
                        is_replay=raw.get("is_replay", False),
                        is_synthetic=raw.get("is_synthetic", False),
                    )
                )
            ]

        case "stream_event":
            evt = raw.get("event", {})
            return [
                StreamDeltaEvent(
                    event_type=evt.get("type", ""),
                    index=evt.get("index"),
                    delta=evt.get("delta"),
                    usage=evt.get("usage"),
                    content_block=evt.get("content_block"),
                )
            ]

        case "compact_boundary":
            meta = raw.get("compact_metadata") or {}
            return [
                ItemCompletedEvent(
                    item=CompactBoundaryItem(
                        id=raw["uuid"],
                        pre_compact_token_count=meta.get(
                            "pre_compact_token_count"
                        ),
                        post_compact_token_count=meta.get(
                            "post_compact_token_count"
                        ),
                    )
                )
            ]

        case "api_retry":
            return [
                SessionErrorEvent(
                    message=raw.get("error", ""),
                    retryable=True,
                    attempt=raw.get("attempt"),
                    max_retries=raw.get("max_retries"),
                    retry_delay_ms=raw.get("retry_delay_ms"),
                )
            ]

        case "tool_use_summary":
            return [
                ItemCompletedEvent(
                    item=ToolUseSummaryItem(
                        id=raw["uuid"],
                        summary=raw["summary"],
                        preceding_tool_use_ids=raw.get(
                            "preceding_tool_use_ids", []
                        ),
                    )
                )
            ]

        case "result":
            return _transform_result(raw)

        case _:
            return []


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _parse_content_block(block: dict[str, Any]) -> ContentBlock:
    """Parse a raw content block dict into a typed dataclass."""
    block_type = block.get("type", "")

    match block_type:
        case "text":
            return TextBlock(type="text", text=block.get("text", ""))
        case "tool_use":
            return ToolUseBlock(
                type="tool_use",
                id=block.get("id", ""),
                name=block.get("name", ""),
                input=block.get("input"),
            )
        case "tool_result":
            return ToolResultBlock(
                type="tool_result",
                tool_use_id=block.get("tool_use_id", ""),
                content=block.get("content"),
                is_error=block.get("is_error", False),
            )
        case "thinking":
            return ThinkingBlock(
                type="thinking",
                thinking=block.get("thinking", ""),
                signature=block.get("signature"),
            )
        case "redacted_thinking":
            return RedactedThinkingBlock(
                type="redacted_thinking",
                data=block.get("data", ""),
            )
        case "image":
            src = block.get("source", {})
            return ImageBlock(
                type="image",
                source=ImageSource(
                    type=src.get("type", ""),
                    media_type=src.get("media_type", ""),
                    data=src.get("data", ""),
                ),
            )
        case _:
            # Unknown block type — treat as text
            return TextBlock(type=block_type, text=str(block))


def _transform_assistant(raw: dict[str, Any]) -> list[SessionEvent]:
    msg = raw["message"]
    raw_blocks: list[dict[str, Any]] = msg.get("content", [])

    # Parse content blocks
    content_blocks = [_parse_content_block(b) for b in raw_blocks]

    # Aggregate text from text blocks
    text = "".join(
        b.get("text", "") for b in raw_blocks if b.get("type") == "text"
    )

    # Parse message-level usage
    raw_usage = msg.get("usage")
    usage: MessageUsage | None = None
    if raw_usage:
        usage = MessageUsage(
            input_tokens=raw_usage.get("input_tokens", 0),
            output_tokens=raw_usage.get("output_tokens", 0),
            cache_read_input_tokens=raw_usage.get(
                "cache_read_input_tokens", 0
            ),
            cache_creation_input_tokens=raw_usage.get(
                "cache_creation_input_tokens", 0
            ),
        )

    item = AgentMessageItem(
        id=msg["uuid"],
        text=text,
        content_blocks=content_blocks,
        usage=usage,
        stop_reason=msg.get("stop_reason"),
        cost_usd=msg.get("cost_usd", 0.0),
    )

    return [ItemCompletedEvent(item=item)]


def _transform_result(raw: dict[str, Any]) -> list[SessionEvent]:
    if raw.get("is_error"):
        errors: list[str] = raw.get("errors", [])
        message = raw.get("result") or "; ".join(errors)
        return [
            TurnFailedEvent(
                error=SessionError(
                    message=message,
                    subtype=raw.get("subtype", "error_during_execution"),
                )
            )
        ]

    raw_usage = raw.get("usage", {})
    usage = Usage(
        input_tokens=raw_usage.get("total_input_tokens", 0),
        cached_input_tokens=raw_usage.get("total_cache_read_tokens", 0),
        output_tokens=raw_usage.get("total_output_tokens", 0),
        total_cost_usd=raw_usage.get("total_cost_usd", 0.0),
    )

    return [
        TurnCompletedEvent(
            usage=usage,
            result=raw.get("result", ""),
            num_turns=raw.get("num_turns", 0),
            duration_ms=raw.get("duration_ms", 0),
        )
    ]

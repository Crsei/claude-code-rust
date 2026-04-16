"""Item types representing content within a session turn.

Mirrors TypeScript SDK's items.ts. Each item corresponds to a specific
``SdkMessage`` variant from the Rust CLI.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Union

# ---------------------------------------------------------------------------
# Content blocks (mirrors Rust ContentBlock enum)
# ---------------------------------------------------------------------------


@dataclass
class TextBlock:
    type: str  # "text"
    text: str


@dataclass
class ToolUseBlock:
    type: str  # "tool_use"
    id: str
    name: str
    input: Any


@dataclass
class ToolResultBlock:
    type: str  # "tool_result"
    tool_use_id: str
    content: Any
    is_error: bool = False


@dataclass
class ThinkingBlock:
    type: str  # "thinking"
    thinking: str
    signature: str | None = None


@dataclass
class RedactedThinkingBlock:
    type: str  # "redacted_thinking"
    data: str


@dataclass
class ImageSource:
    type: str
    media_type: str
    data: str


@dataclass
class ImageBlock:
    type: str  # "image"
    source: ImageSource


ContentBlock = Union[
    TextBlock,
    ToolUseBlock,
    ToolResultBlock,
    ThinkingBlock,
    RedactedThinkingBlock,
    ImageBlock,
]

# ---------------------------------------------------------------------------
# Message-level usage
# ---------------------------------------------------------------------------


@dataclass
class MessageUsage:
    input_tokens: int = 0
    output_tokens: int = 0
    cache_read_input_tokens: int = 0
    cache_creation_input_tokens: int = 0


# ---------------------------------------------------------------------------
# Item types
# ---------------------------------------------------------------------------


@dataclass
class AgentMessageItem:
    """Agent message — from ``SdkMessage::Assistant``."""

    id: str
    type: str = "agent_message"
    text: str = ""
    content_blocks: list[ContentBlock] = field(default_factory=list)
    usage: MessageUsage | None = None
    stop_reason: str | None = None
    cost_usd: float = 0.0


@dataclass
class ToolUseSummaryItem:
    """Tool-use summary — from ``SdkMessage::ToolUseSummary``."""

    id: str
    type: str = "tool_use_summary"
    summary: str = ""
    preceding_tool_use_ids: list[str] = field(default_factory=list)


@dataclass
class CompactBoundaryItem:
    """Compact boundary — from ``SdkMessage::CompactBoundary``."""

    id: str
    type: str = "compact_boundary"
    pre_compact_token_count: int | None = None
    post_compact_token_count: int | None = None


@dataclass
class UserReplayItem:
    """User message replay — from ``SdkMessage::UserReplay``."""

    id: str
    type: str = "user_replay"
    content: str = ""
    is_replay: bool = False
    is_synthetic: bool = False


@dataclass
class ErrorItem:
    """Error item."""

    id: str
    type: str = "error"
    message: str = ""


SessionItem = Union[
    AgentMessageItem,
    ToolUseSummaryItem,
    CompactBoundaryItem,
    UserReplayItem,
    ErrorItem,
]

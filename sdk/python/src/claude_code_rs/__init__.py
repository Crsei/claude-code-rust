"""claude-code-rs Python SDK

A thin wrapper around the ``claude-code-rs`` CLI binary that provides
a typed, streaming interface for programmatic interaction.
"""

__version__ = "0.1.0"

# Classes
from .claude_code import ClaudeCode
from .session import Session

# Result types
from .session import Turn, StreamedTurn

# Event types
from .events import (
    SessionEvent,
    SessionStartedEvent,
    TurnStartedEvent,
    TurnCompletedEvent,
    TurnFailedEvent,
    ItemStartedEvent,
    ItemUpdatedEvent,
    ItemCompletedEvent,
    StreamDeltaEvent,
    SessionErrorEvent,
    Usage,
    SessionError,
)

# Item types
from .items import (
    SessionItem,
    AgentMessageItem,
    ToolUseSummaryItem,
    CompactBoundaryItem,
    UserReplayItem,
    ErrorItem,
    ContentBlock,
    TextBlock,
    ToolUseBlock,
    ToolResultBlock,
    ThinkingBlock,
    RedactedThinkingBlock,
    ImageBlock,
    ImageSource,
    MessageUsage,
)

# Option types
from .config import ClientOptions, SessionOptions, PermissionMode

# Errors
from .errors import (
    CcRustError,
    BinaryNotFoundError,
    ProcessError,
    TurnExecutionError,
    ParseError,
)

__all__ = [
    # Classes
    "ClaudeCode",
    "Session",
    # Result types
    "Turn",
    "StreamedTurn",
    # Event types
    "SessionEvent",
    "SessionStartedEvent",
    "TurnStartedEvent",
    "TurnCompletedEvent",
    "TurnFailedEvent",
    "ItemStartedEvent",
    "ItemUpdatedEvent",
    "ItemCompletedEvent",
    "StreamDeltaEvent",
    "SessionErrorEvent",
    "Usage",
    "SessionError",
    # Item types
    "SessionItem",
    "AgentMessageItem",
    "ToolUseSummaryItem",
    "CompactBoundaryItem",
    "UserReplayItem",
    "ErrorItem",
    "ContentBlock",
    "TextBlock",
    "ToolUseBlock",
    "ToolResultBlock",
    "ThinkingBlock",
    "RedactedThinkingBlock",
    "ImageBlock",
    "ImageSource",
    "MessageUsage",
    # Options
    "ClientOptions",
    "SessionOptions",
    "PermissionMode",
    # Errors
    "CcRustError",
    "BinaryNotFoundError",
    "ProcessError",
    "TurnExecutionError",
    "ParseError",
]

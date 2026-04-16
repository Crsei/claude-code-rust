/**
 * claude-code-rs TypeScript SDK
 *
 * A thin wrapper around the `claude-code-rs` CLI binary that provides
 * a typed, streaming interface for programmatic interaction.
 */

// Classes
export { ClaudeCode } from "./claudeCode.js";
export { Session } from "./session.js";

// Result types
export type { Turn, StreamedTurn } from "./session.js";

// Event types
export type {
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
} from "./events.js";

// Item types
export type {
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
  ImageSource,
  MessageUsage,
} from "./items.js";

// Option types
export type { ClaudeCodeOptions } from "./claudeCodeOptions.js";
export type { SessionOptions, PermissionMode } from "./sessionOptions.js";
export type { TurnOptions } from "./turnOptions.js";

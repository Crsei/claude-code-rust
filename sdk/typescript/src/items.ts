/**
 * Item types representing content within a session turn.
 *
 * Each item corresponds to a specific `SdkMessage` variant from the Rust CLI.
 */

// ---------------------------------------------------------------------------
// Content blocks (mirrors Rust ContentBlock enum)
// ---------------------------------------------------------------------------

export type TextBlock = { type: "text"; text: string };
export type ToolUseBlock = {
  type: "tool_use";
  id: string;
  name: string;
  input: unknown;
};
export type ToolResultBlock = {
  type: "tool_result";
  tool_use_id: string;
  content: unknown;
  is_error: boolean;
};
export type ThinkingBlock = {
  type: "thinking";
  thinking: string;
  signature?: string;
};
export type RedactedThinkingBlock = { type: "redacted_thinking"; data: string };
export type ImageBlock = { type: "image"; source: ImageSource };

export type ContentBlock =
  | TextBlock
  | ToolUseBlock
  | ToolResultBlock
  | ThinkingBlock
  | RedactedThinkingBlock
  | ImageBlock;

export type ImageSource = {
  type: string;
  media_type: string;
  data: string;
};

// ---------------------------------------------------------------------------
// Message-level usage
// ---------------------------------------------------------------------------

export type MessageUsage = {
  input_tokens: number;
  output_tokens: number;
  cache_read_input_tokens: number;
  cache_creation_input_tokens: number;
};

// ---------------------------------------------------------------------------
// Item types
// ---------------------------------------------------------------------------

/** Agent message — from `SdkMessage::Assistant`. */
export type AgentMessageItem = {
  id: string;
  type: "agent_message";
  text: string;
  content_blocks: ContentBlock[];
  usage?: MessageUsage;
  stop_reason?: string;
  cost_usd: number;
};

/** Tool-use summary — from `SdkMessage::ToolUseSummary`. */
export type ToolUseSummaryItem = {
  id: string;
  type: "tool_use_summary";
  summary: string;
  preceding_tool_use_ids: string[];
};

/** Compact boundary — from `SdkMessage::CompactBoundary`. */
export type CompactBoundaryItem = {
  id: string;
  type: "compact_boundary";
  pre_compact_token_count?: number;
  post_compact_token_count?: number;
};

/** User message replay — from `SdkMessage::UserReplay`. */
export type UserReplayItem = {
  id: string;
  type: "user_replay";
  content: string;
  is_replay: boolean;
  is_synthetic: boolean;
};

/** Error item. */
export type ErrorItem = {
  id: string;
  type: "error";
  message: string;
};

// ---------------------------------------------------------------------------
// Union
// ---------------------------------------------------------------------------

export type SessionItem =
  | AgentMessageItem
  | ToolUseSummaryItem
  | CompactBoundaryItem
  | UserReplayItem
  | ErrorItem;

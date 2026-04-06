/**
 * Transforms raw JSONL from `claude-code-rs --output-format json` into
 * normalized `SessionEvent` types.
 *
 * The raw JSONL matches the Rust `SdkMessage` serde serialization
 * (internally tagged with `#[serde(tag = "type", rename_all = "snake_case")]`).
 */

import type { SessionEvent, Usage } from "./events.js";
import type {
  AgentMessageItem,
  ContentBlock,
  MessageUsage,
  SessionItem,
} from "./items.js";

// ---------------------------------------------------------------------------
// Raw JSONL shapes (matching Rust SdkMessage serde output)
// ---------------------------------------------------------------------------

/** @internal */
export type RawSdkMessage =
  | RawSystemInit
  | RawAssistant
  | RawUserReplay
  | RawStreamEvent
  | RawCompactBoundary
  | RawApiRetry
  | RawToolUseSummary
  | RawResult;

type RawSystemInit = {
  type: "system_init";
  tools: string[];
  model: string;
  permission_mode: string;
  session_id: string;
  uuid: string;
};

type RawAssistant = {
  type: "assistant";
  message: {
    uuid: string;
    timestamp: number;
    role: string;
    content: ContentBlock[];
    usage?: MessageUsage;
    stop_reason?: string;
    is_api_error_message: boolean;
    api_error?: string;
    cost_usd: number;
  };
  session_id: string;
  parent_tool_use_id?: string;
};

type RawUserReplay = {
  type: "user_replay";
  content: string;
  session_id: string;
  uuid: string;
  timestamp: number;
  is_replay: boolean;
  is_synthetic: boolean;
};

type RawStreamEvent = {
  type: "stream_event";
  event: {
    type: string;
    index?: number;
    delta?: unknown;
    usage?: unknown;
    content_block?: unknown;
  };
  session_id: string;
  uuid: string;
};

type RawCompactBoundary = {
  type: "compact_boundary";
  session_id: string;
  uuid: string;
  compact_metadata?: {
    pre_compact_token_count: number;
    post_compact_token_count: number;
  };
};

type RawApiRetry = {
  type: "api_retry";
  attempt: number;
  max_retries: number;
  retry_delay_ms: number;
  error_status?: number;
  error: string;
  session_id: string;
  uuid: string;
};

type RawToolUseSummary = {
  type: "tool_use_summary";
  summary: string;
  preceding_tool_use_ids: string[];
  session_id: string;
  uuid: string;
};

type RawResult = {
  type: "result";
  subtype: string;
  is_error: boolean;
  duration_ms: number;
  duration_api_ms: number;
  num_turns: number;
  result: string;
  stop_reason?: string;
  session_id: string;
  total_cost_usd: number;
  usage: {
    total_input_tokens: number;
    total_output_tokens: number;
    total_cache_read_tokens: number;
    total_cache_creation_tokens: number;
    total_cost_usd: number;
    api_call_count: number;
  };
  permission_denials: unknown[];
  structured_output?: unknown;
  uuid: string;
  errors: string[];
};

// ---------------------------------------------------------------------------
// Transform
// ---------------------------------------------------------------------------

export function transformRawEvent(raw: RawSdkMessage): SessionEvent[] {
  switch (raw.type) {
    case "system_init":
      return [
        {
          type: "session.started",
          session_id: raw.session_id,
          model: raw.model,
          tools: raw.tools,
          permission_mode: raw.permission_mode,
        },
      ];

    case "assistant":
      return transformAssistant(raw);

    case "user_replay":
      return [
        {
          type: "item.completed",
          item: {
            id: raw.uuid,
            type: "user_replay",
            content: raw.content,
            is_replay: raw.is_replay,
            is_synthetic: raw.is_synthetic,
          },
        },
      ];

    case "stream_event":
      return [
        {
          type: "stream.delta",
          event_type: raw.event.type,
          index: raw.event.index,
          delta: raw.event.delta,
          usage: raw.event.usage,
          content_block: raw.event.content_block,
        },
      ];

    case "compact_boundary":
      return [
        {
          type: "item.completed",
          item: {
            id: raw.uuid,
            type: "compact_boundary",
            pre_compact_token_count:
              raw.compact_metadata?.pre_compact_token_count,
            post_compact_token_count:
              raw.compact_metadata?.post_compact_token_count,
          },
        },
      ];

    case "api_retry":
      return [
        {
          type: "error",
          message: raw.error,
          retryable: true,
          attempt: raw.attempt,
          max_retries: raw.max_retries,
          retry_delay_ms: raw.retry_delay_ms,
        },
      ];

    case "tool_use_summary":
      return [
        {
          type: "item.completed",
          item: {
            id: raw.uuid,
            type: "tool_use_summary",
            summary: raw.summary,
            preceding_tool_use_ids: raw.preceding_tool_use_ids,
          },
        },
      ];

    case "result":
      return transformResult(raw);

    default:
      return [];
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function transformAssistant(raw: RawAssistant): SessionEvent[] {
  const msg = raw.message;

  // Extract text from content blocks
  const text = msg.content
    .filter((b): b is { type: "text"; text: string } => b.type === "text")
    .map((b) => b.text)
    .join("");

  const item: AgentMessageItem = {
    id: msg.uuid,
    type: "agent_message",
    text,
    content_blocks: msg.content,
    usage: msg.usage,
    stop_reason: msg.stop_reason,
    cost_usd: msg.cost_usd,
  };

  return [{ type: "item.completed", item }];
}

function transformResult(raw: RawResult): SessionEvent[] {
  if (raw.is_error) {
    return [
      {
        type: "turn.failed" as const,
        error: {
          message: raw.result || raw.errors.join("; "),
          subtype: raw.subtype as
            | "error_during_execution"
            | "error_max_turns"
            | "error_max_budget_usd"
            | "error_max_structured_output_retries",
        },
      },
    ];
  }

  const usage: Usage = {
    input_tokens: raw.usage.total_input_tokens,
    cached_input_tokens: raw.usage.total_cache_read_tokens,
    output_tokens: raw.usage.total_output_tokens,
    total_cost_usd: raw.usage.total_cost_usd,
  };

  return [
    {
      type: "turn.completed",
      usage,
      result: raw.result,
      num_turns: raw.num_turns,
      duration_ms: raw.duration_ms,
    },
  ];
}

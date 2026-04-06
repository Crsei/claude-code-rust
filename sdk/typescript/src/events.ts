/**
 * Normalized SDK event types.
 *
 * These are the consumer-facing events yielded by `Session.runStreamed()`.
 * Raw Rust JSONL events are transformed into these types by `transform.ts`.
 */

import type { SessionItem } from "./items.js";

// ---------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------

export type Usage = {
  input_tokens: number;
  cached_input_tokens: number;
  output_tokens: number;
  total_cost_usd: number;
};

// ---------------------------------------------------------------------------
// Session lifecycle events
// ---------------------------------------------------------------------------

export type SessionStartedEvent = {
  type: "session.started";
  session_id: string;
  model: string;
  tools: string[];
  permission_mode: string;
};

export type TurnStartedEvent = {
  type: "turn.started";
};

export type TurnCompletedEvent = {
  type: "turn.completed";
  usage: Usage;
  result: string;
  num_turns: number;
  duration_ms: number;
};

export type SessionError = {
  message: string;
  subtype:
    | "error_during_execution"
    | "error_max_turns"
    | "error_max_budget_usd"
    | "error_max_structured_output_retries";
};

export type TurnFailedEvent = {
  type: "turn.failed";
  error: SessionError;
};

// ---------------------------------------------------------------------------
// Item events
// ---------------------------------------------------------------------------

export type ItemStartedEvent = {
  type: "item.started";
  item: SessionItem;
};

export type ItemUpdatedEvent = {
  type: "item.updated";
  item: SessionItem;
};

export type ItemCompletedEvent = {
  type: "item.completed";
  item: SessionItem;
};

// ---------------------------------------------------------------------------
// Stream delta (raw pass-through of streaming content)
// ---------------------------------------------------------------------------

export type StreamDeltaEvent = {
  type: "stream.delta";
  event_type: string;
  index?: number;
  delta?: unknown;
  usage?: unknown;
  content_block?: unknown;
};

// ---------------------------------------------------------------------------
// Error event (retryable API errors)
// ---------------------------------------------------------------------------

export type SessionErrorEvent = {
  type: "error";
  message: string;
  retryable: boolean;
  attempt?: number;
  max_retries?: number;
  retry_delay_ms?: number;
};

// ---------------------------------------------------------------------------
// Top-level union
// ---------------------------------------------------------------------------

export type SessionEvent =
  | SessionStartedEvent
  | TurnStartedEvent
  | TurnCompletedEvent
  | TurnFailedEvent
  | ItemStartedEvent
  | ItemUpdatedEvent
  | ItemCompletedEvent
  | StreamDeltaEvent
  | SessionErrorEvent;

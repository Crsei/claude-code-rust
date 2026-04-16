/**
 * Mock child process for unit testing the JSONL parsing pipeline.
 *
 * Creates a fake child process that emits pre-configured JSONL lines on stdout.
 */

import { EventEmitter } from "node:events";
import { PassThrough } from "node:stream";

export class FakeChildProcess extends EventEmitter {
  stdin = new PassThrough();
  stdout = new PassThrough();
  stderr = new PassThrough();
  killed = false;

  kill() {
    this.killed = true;
  }
}

/**
 * Create a mock child process that emits the given objects as JSONL lines,
 * then exits with the specified code.
 */
export function createMockProcess(
  jsonlLines: object[],
  exitCode = 0,
): FakeChildProcess {
  const child = new FakeChildProcess();

  setImmediate(() => {
    for (const line of jsonlLines) {
      child.stdout.write(JSON.stringify(line) + "\n");
    }
    child.stdout.end();
    child.emit("exit", exitCode, null);
  });

  return child;
}

// ---------------------------------------------------------------------------
// Sample JSONL payloads (matching Rust SdkMessage serde output)
// ---------------------------------------------------------------------------

export const sampleSystemInit = {
  type: "system_init",
  tools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
  model: "claude-sonnet-4-20250514",
  permission_mode: "default",
  session_id: "test-session-123",
  uuid: "00000000-0000-0000-0000-000000000001",
};

export const sampleAssistant = {
  type: "assistant",
  message: {
    uuid: "00000000-0000-0000-0000-000000000002",
    timestamp: 1700000000,
    role: "assistant",
    content: [{ type: "text", text: "Hello! I can help with that." }],
    usage: {
      input_tokens: 100,
      output_tokens: 20,
      cache_read_input_tokens: 50,
      cache_creation_input_tokens: 0,
    },
    stop_reason: "end_turn",
    is_api_error_message: false,
    api_error: null,
    cost_usd: 0.001,
  },
  session_id: "test-session-123",
  parent_tool_use_id: null,
};

export const sampleStreamEvent = {
  type: "stream_event",
  event: {
    type: "content_block_delta",
    index: 0,
    delta: { type: "text_delta", text: "Hello" },
  },
  session_id: "test-session-123",
  uuid: "00000000-0000-0000-0000-000000000003",
};

export const sampleToolUseSummary = {
  type: "tool_use_summary",
  summary: "Read file src/main.rs",
  preceding_tool_use_ids: ["tool-1"],
  session_id: "test-session-123",
  uuid: "00000000-0000-0000-0000-000000000004",
};

export const sampleApiRetry = {
  type: "api_retry",
  attempt: 1,
  max_retries: 3,
  retry_delay_ms: 1000,
  error_status: 429,
  error: "Rate limited",
  session_id: "test-session-123",
  uuid: "00000000-0000-0000-0000-000000000005",
};

export const sampleResult = {
  type: "result",
  subtype: "success",
  is_error: false,
  duration_ms: 5000,
  duration_api_ms: 4500,
  num_turns: 1,
  result: "Hello! I can help with that.",
  stop_reason: "end_turn",
  session_id: "test-session-123",
  total_cost_usd: 0.001,
  usage: {
    total_input_tokens: 100,
    total_output_tokens: 20,
    total_cache_read_tokens: 50,
    total_cache_creation_tokens: 0,
    total_cost_usd: 0.001,
    api_call_count: 1,
  },
  permission_denials: [],
  structured_output: null,
  uuid: "00000000-0000-0000-0000-000000000006",
  errors: [],
};

export const sampleErrorResult = {
  ...sampleResult,
  subtype: "error_during_execution",
  is_error: true,
  result: "An error occurred during execution",
  errors: ["An error occurred during execution"],
};

export const sampleCompactBoundary = {
  type: "compact_boundary",
  session_id: "test-session-123",
  uuid: "00000000-0000-0000-0000-000000000007",
  compact_metadata: {
    pre_compact_token_count: 50000,
    post_compact_token_count: 10000,
  },
};

export const sampleUserReplay = {
  type: "user_replay",
  content: "What files are here?",
  session_id: "test-session-123",
  uuid: "00000000-0000-0000-0000-000000000008",
  timestamp: 1700000000,
  is_replay: true,
  is_synthetic: false,
};

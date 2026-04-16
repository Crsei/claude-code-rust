import { transformRawEvent, type RawSdkMessage } from "../src/transform.js";
import {
  sampleSystemInit,
  sampleAssistant,
  sampleStreamEvent,
  sampleToolUseSummary,
  sampleApiRetry,
  sampleResult,
  sampleErrorResult,
  sampleCompactBoundary,
  sampleUserReplay,
} from "./mockProcess.js";

describe("transformRawEvent", () => {
  it("transforms system_init to session.started", () => {
    const events = transformRawEvent(sampleSystemInit as RawSdkMessage);
    expect(events).toHaveLength(1);
    expect(events[0]).toEqual({
      type: "session.started",
      session_id: "test-session-123",
      model: "claude-sonnet-4-20250514",
      tools: ["Bash", "Read", "Write", "Edit", "Glob", "Grep"],
      permission_mode: "default",
    });
  });

  it("transforms assistant to item.completed with agent_message", () => {
    const events = transformRawEvent(sampleAssistant as RawSdkMessage);
    expect(events).toHaveLength(1);
    const event = events[0]!;
    expect(event.type).toBe("item.completed");
    if (event.type === "item.completed") {
      expect(event.item.type).toBe("agent_message");
      if (event.item.type === "agent_message") {
        expect(event.item.text).toBe("Hello! I can help with that.");
        expect(event.item.content_blocks).toHaveLength(1);
        expect(event.item.cost_usd).toBe(0.001);
      }
    }
  });

  it("transforms stream_event to stream.delta", () => {
    const events = transformRawEvent(sampleStreamEvent as RawSdkMessage);
    expect(events).toHaveLength(1);
    expect(events[0]).toEqual({
      type: "stream.delta",
      event_type: "content_block_delta",
      index: 0,
      delta: { type: "text_delta", text: "Hello" },
      usage: undefined,
      content_block: undefined,
    });
  });

  it("transforms tool_use_summary to item.completed", () => {
    const events = transformRawEvent(sampleToolUseSummary as RawSdkMessage);
    expect(events).toHaveLength(1);
    const event = events[0]!;
    expect(event.type).toBe("item.completed");
    if (event.type === "item.completed") {
      expect(event.item.type).toBe("tool_use_summary");
      if (event.item.type === "tool_use_summary") {
        expect(event.item.summary).toBe("Read file src/main.rs");
        expect(event.item.preceding_tool_use_ids).toEqual(["tool-1"]);
      }
    }
  });

  it("transforms api_retry to error event", () => {
    const events = transformRawEvent(sampleApiRetry as RawSdkMessage);
    expect(events).toHaveLength(1);
    expect(events[0]).toEqual({
      type: "error",
      message: "Rate limited",
      retryable: true,
      attempt: 1,
      max_retries: 3,
      retry_delay_ms: 1000,
    });
  });

  it("transforms successful result to turn.completed", () => {
    const events = transformRawEvent(sampleResult as RawSdkMessage);
    expect(events).toHaveLength(1);
    const event = events[0]!;
    expect(event.type).toBe("turn.completed");
    if (event.type === "turn.completed") {
      expect(event.result).toBe("Hello! I can help with that.");
      expect(event.num_turns).toBe(1);
      expect(event.duration_ms).toBe(5000);
      expect(event.usage).toEqual({
        input_tokens: 100,
        cached_input_tokens: 50,
        output_tokens: 20,
        total_cost_usd: 0.001,
      });
    }
  });

  it("transforms error result to turn.failed", () => {
    const events = transformRawEvent(sampleErrorResult as RawSdkMessage);
    expect(events).toHaveLength(1);
    const event = events[0]!;
    expect(event.type).toBe("turn.failed");
    if (event.type === "turn.failed") {
      expect(event.error.message).toBe("An error occurred during execution");
      expect(event.error.subtype).toBe("error_during_execution");
    }
  });

  it("transforms compact_boundary to item.completed", () => {
    const events = transformRawEvent(sampleCompactBoundary as RawSdkMessage);
    expect(events).toHaveLength(1);
    const event = events[0]!;
    expect(event.type).toBe("item.completed");
    if (event.type === "item.completed") {
      expect(event.item.type).toBe("compact_boundary");
      if (event.item.type === "compact_boundary") {
        expect(event.item.pre_compact_token_count).toBe(50000);
        expect(event.item.post_compact_token_count).toBe(10000);
      }
    }
  });

  it("transforms user_replay to item.completed", () => {
    const events = transformRawEvent(sampleUserReplay as RawSdkMessage);
    expect(events).toHaveLength(1);
    const event = events[0]!;
    expect(event.type).toBe("item.completed");
    if (event.type === "item.completed") {
      expect(event.item.type).toBe("user_replay");
      if (event.item.type === "user_replay") {
        expect(event.item.content).toBe("What files are here?");
        expect(event.item.is_replay).toBe(true);
        expect(event.item.is_synthetic).toBe(false);
      }
    }
  });

  it("returns empty array for unknown type", () => {
    const events = transformRawEvent({ type: "unknown_type" } as any);
    expect(events).toEqual([]);
  });
});

/**
 * Session — represents a conversation with the `claude-code-rs` agent.
 *
 * Analogous to `Thread` in the Codex SDK.
 */

import type { ClaudeCodeOptions } from "./claudeCodeOptions.js";
import type { SessionEvent, Usage } from "./events.js";
import type { ClaudeCodeExec } from "./exec.js";
import type { SessionItem } from "./items.js";
import type { SessionOptions } from "./sessionOptions.js";
import type { TurnOptions } from "./turnOptions.js";
import { transformRawEvent, type RawSdkMessage } from "./transform.js";

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

export type Turn = {
  items: SessionItem[];
  finalResponse: string;
  usage: Usage | null;
};

export type StreamedTurn = {
  events: AsyncGenerator<SessionEvent>;
};

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

export class Session {
  private _exec: ClaudeCodeExec;
  private _clientOptions: ClaudeCodeOptions;
  private _sessionOptions: SessionOptions;
  private _sessionId: string | null = null;

  /** Session ID — populated after the first `system_init` event. */
  get sessionId(): string | null {
    return this._sessionId;
  }

  /** @internal */
  constructor(
    exec: ClaudeCodeExec,
    clientOptions: ClaudeCodeOptions,
    sessionOptions: SessionOptions,
  ) {
    this._exec = exec;
    this._clientOptions = clientOptions;
    this._sessionOptions = sessionOptions;
  }

  /**
   * Execute a turn and return a buffered result.
   *
   * Consumes the entire event stream and collects items.
   */
  async run(input: string, turnOptions?: TurnOptions): Promise<Turn> {
    const { events } = await this.runStreamed(input, turnOptions);

    const items: SessionItem[] = [];
    let finalResponse = "";
    let usage: Usage | null = null;
    let turnFailure: string | null = null;

    for await (const event of events) {
      switch (event.type) {
        case "item.completed":
          items.push(event.item);
          if (event.item.type === "agent_message") {
            finalResponse = event.item.text;
          }
          break;
        case "turn.completed":
          usage = event.usage;
          break;
        case "turn.failed":
          turnFailure = event.error.message;
          break;
      }
    }

    if (turnFailure) {
      throw new Error(turnFailure);
    }

    return { items, finalResponse, usage };
  }

  /**
   * Execute a turn and stream events as an async generator.
   */
  async runStreamed(
    input: string,
    turnOptions?: TurnOptions,
  ): Promise<StreamedTurn> {
    return { events: this.runStreamedInternal(input, turnOptions) };
  }

  private async *runStreamedInternal(
    input: string,
    turnOptions?: TurnOptions,
  ): AsyncGenerator<SessionEvent> {
    const generator = this._exec.run({
      input,
      apiKey: this._clientOptions.apiKey,
      model: this._sessionOptions.model,
      workingDirectory: this._sessionOptions.workingDirectory,
      permissionMode: this._sessionOptions.permissionMode,
      maxTurns: this._sessionOptions.maxTurns,
      maxBudget: this._sessionOptions.maxBudget,
      systemPrompt: this._sessionOptions.systemPrompt,
      appendSystemPrompt: this._sessionOptions.appendSystemPrompt,
      verbose: this._sessionOptions.verbose,
      continueSession: this._sessionOptions.continueSession,
      signal: turnOptions?.signal,
    });

    try {
      for await (const line of generator) {
        let raw: RawSdkMessage;
        try {
          raw = JSON.parse(line) as RawSdkMessage;
        } catch (error) {
          throw new Error(`Failed to parse JSONL line: ${line}`, {
            cause: error,
          });
        }

        const events = transformRawEvent(raw);
        for (const event of events) {
          // Capture session ID from the first system_init event
          if (
            event.type === "session.started" &&
            this._sessionId === null
          ) {
            this._sessionId = event.session_id;
          }
          yield event;
        }
      }
    } finally {
      // Generator cleanup is handled by exec.ts finally block
    }
  }
}

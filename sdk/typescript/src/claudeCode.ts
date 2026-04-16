/**
 * Top-level client — analogous to `Codex` in the Codex SDK.
 *
 * Usage:
 * ```ts
 * const client = new ClaudeCode();
 * const session = client.startSession({ model: "claude-sonnet-4-20250514" });
 * const turn = await session.run("What files are in this directory?");
 * console.log(turn.finalResponse);
 * ```
 */

import type { ClaudeCodeOptions } from "./claudeCodeOptions.js";
import { ClaudeCodeExec } from "./exec.js";
import { Session } from "./session.js";
import type { SessionOptions } from "./sessionOptions.js";

export class ClaudeCode {
  private exec: ClaudeCodeExec;
  private options: ClaudeCodeOptions;

  constructor(options: ClaudeCodeOptions = {}) {
    this.exec = new ClaudeCodeExec(
      options.executablePath ?? null,
      options.env,
    );
    this.options = options;
  }

  /** Create a new session. */
  startSession(options: SessionOptions = {}): Session {
    return new Session(this.exec, this.options, options);
  }

  /** Resume a previously persisted session by ID. */
  resumeSession(sessionId: string, options: SessionOptions = {}): Session {
    return new Session(this.exec, this.options, {
      ...options,
      continueSession: sessionId,
    });
  }
}

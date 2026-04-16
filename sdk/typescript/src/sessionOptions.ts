/**
 * Options for creating or resuming a session.
 *
 * Maps to `claude-code-rs` CLI arguments.
 */

export type PermissionMode = "default" | "auto" | "bypass" | "plan";

export type SessionOptions = {
  /** Model override (`--model`). */
  model?: string;

  /** Working directory (`--cwd`). */
  workingDirectory?: string;

  /** Permission mode (`--permission-mode`). */
  permissionMode?: PermissionMode;

  /** Maximum number of turns for agentic loops (`--max-turns`). */
  maxTurns?: number;

  /** Maximum budget in USD (`--max-budget`). */
  maxBudget?: number;

  /** Custom system prompt — replaces default (`--system-prompt`). */
  systemPrompt?: string;

  /** Append to the system prompt (`--append-system-prompt`). */
  appendSystemPrompt?: string;

  /** Enable verbose output (`--verbose`). */
  verbose?: boolean;

  /** Resume a specific session by ID (`--continue`). */
  continueSession?: string;
};

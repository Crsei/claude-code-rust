/**
 * Options for the top-level `ClaudeCode` client.
 */
export type ClaudeCodeOptions = {
  /** Path to the `claude-code-rs` binary. Auto-detected if omitted. */
  executablePath?: string;

  /** API key (passed as `ANTHROPIC_API_KEY` env var to the subprocess). */
  apiKey?: string;

  /** Environment variables passed to the CLI process. */
  env?: Record<string, string>;
};

import path from "node:path";

/**
 * Resolve the path to the `claude-code-rs` binary for tests.
 */
export function executablePath(): string {
  return (
    process.env["CLAUDE_CODE_RS_PATH"] ??
    path.resolve(__dirname, "..", "..", "..", "target", "debug", "claude-code-rs")
  );
}

import React from 'react'
import { c } from '../../theme.js'
import { formatFileSize, stripAnsi } from './format.js'
import { ShellTimeDisplay } from './ShellTimeDisplay.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/shell/ShellProgressMessage.tsx`.
 *
 * Renders the live/trailing view of a shell (Bash) command as it runs:
 * - While there is no output yet: dim "Running…" line + elapsed/timeout.
 * - Once output starts flowing: dim tail (last 5 lines by default) plus
 *   a status row with `+N lines`, elapsed, and total bytes.
 * - In verbose mode the full output is shown with no tail cap.
 *
 * Backing data comes from a `tool_progress` IPC event emitted by the
 * Rust `BashTool` (see `crates/claude-code-rs/src/tools/exec/bash.rs`).
 * The upstream component wraps itself in `OffscreenFreeze` to avoid
 * terminal resets when the progress line scrolls into scrollback —
 * OpenTUI does its own diffing so we don't need the same guard here.
 */

type Props = {
  /** Most-recent output (may already be tail-capped by the backend). */
  output: string
  /** Full accumulated output — used in verbose mode. */
  fullOutput: string
  /** Whole-seconds the command has been running. */
  elapsedTimeSeconds?: number
  /** Total stdout+stderr line count (including tail-capped portion). */
  totalLines?: number
  /** Total stdout+stderr byte count. */
  totalBytes?: number
  /** Configured timeout in milliseconds. */
  timeoutMs?: number
  /** Show every output line instead of a 5-line tail. */
  verbose: boolean
  /** Max tail lines when not verbose. Upstream uses 5. */
  tailLines?: number
}

export function ShellProgressMessage({
  output,
  fullOutput,
  elapsedTimeSeconds,
  totalLines,
  totalBytes,
  timeoutMs,
  verbose,
  tailLines = 5,
}: Props) {
  const strippedFullOutput = stripAnsi(fullOutput.trim())
  const strippedOutput = stripAnsi(output.trim())
  const lines = strippedOutput.split('\n').filter(line => line)
  const displayLines = verbose
    ? strippedFullOutput
    : lines.slice(-tailLines).join('\n')

  if (!lines.length) {
    return (
      <box flexDirection="row" gap={1}>
        <text fg={c.dim}>{'Running\u2026'}</text>
        <ShellTimeDisplay
          elapsedTimeSeconds={elapsedTimeSeconds}
          timeoutMs={timeoutMs}
        />
      </box>
    )
  }

  // Upstream rules:
  //  - Truncated tail (backend capped): `~<totalLines> lines`
  //  - Not truncated but more lines than shown: `+<extra> lines`
  const extraLines = totalLines ? Math.max(0, totalLines - tailLines) : 0
  let lineStatus = ''
  if (!verbose && totalBytes && totalLines) {
    lineStatus = `~${totalLines} lines`
  } else if (!verbose && extraLines > 0) {
    lineStatus = `+${extraLines} lines`
  }

  return (
    <box flexDirection="column">
      <box
        flexDirection="column"
        height={verbose ? undefined : Math.min(tailLines, lines.length)}
        overflow="hidden"
      >
        <text fg={c.dim}>{displayLines}</text>
      </box>
      <box flexDirection="row" gap={1}>
        {lineStatus ? <text fg={c.dim}>{lineStatus}</text> : null}
        <ShellTimeDisplay
          elapsedTimeSeconds={elapsedTimeSeconds}
          timeoutMs={timeoutMs}
        />
        {totalBytes ? (
          <text fg={c.dim}>{formatFileSize(totalBytes)}</text>
        ) : null}
      </box>
    </box>
  )
}

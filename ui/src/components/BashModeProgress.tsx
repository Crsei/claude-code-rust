import React from 'react'
import { c } from '../theme.js'
import { ShellProgressMessage } from './shell/index.js'

/**
 * Progress chrome for inline `!bash-mode` commands.
 *
 * OpenTUI-native port of the upstream `BashModeProgress`
 * (`ui/examples/upstream-patterns/src/components/BashModeProgress.tsx`).
 * Upstream composed `UserBashInputMessage` + `ShellProgressMessage`; the
 * Lite tree keeps a simpler `<bash-input>` preview (the prompt bubble
 * renders the user-facing `!…` already) and delegates live progress
 * to the existing `ShellProgressMessage` port in `components/shell/`.
 */

export interface ShellProgressSnapshot {
  /** Most-recent (possibly tail-capped) output. */
  output: string
  /** Full accumulated output — used in verbose mode. */
  fullOutput: string
  elapsedTimeSeconds: number
  totalLines?: number
  totalBytes?: number
  timeoutMs?: number
}

type Props = {
  /** The command the user typed after the `!` bash-mode prefix. */
  input: string
  /** Latest progress snapshot, or null before the tool has emitted any. */
  progress: ShellProgressSnapshot | null
  /** Render the full output instead of the tail cap. */
  verbose: boolean
}

export function BashModeProgress({ input, progress, verbose }: Props) {
  return (
    <box flexDirection="column" marginTop={1}>
      <text>
        <span fg={c.warning}>$ </span>
        <span>{input}</span>
      </text>
      {progress ? (
        <ShellProgressMessage
          output={progress.output}
          fullOutput={progress.fullOutput}
          elapsedTimeSeconds={progress.elapsedTimeSeconds}
          totalLines={progress.totalLines}
          totalBytes={progress.totalBytes}
          timeoutMs={progress.timeoutMs}
          verbose={verbose}
        />
      ) : (
        <text fg={c.dim}>{'Running\u2026'}</text>
      )}
    </box>
  )
}

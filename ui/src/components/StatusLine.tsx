import React from 'react'
import type { ViewMode } from '../keybindings.js'
import { useAppState } from '../store/app-store.js'
import type { Usage } from '../store/app-store.js'
import { BuiltinStatusLine } from './StatusLine/BuiltinStatusLine.js'
import { CustomStatusLine } from './StatusLine/CustomStatusLine.js'
import { shouldRenderCustomStatusLine } from './StatusLine/status-line-state.js'

/**
 * Orchestrator that composes the built-in statusline with the optional
 * user-configured custom statusline. Ported from
 * `ui/examples/upstream-patterns/src/components/StatusLine.tsx`.
 * The upstream relies on Ink's `ANSI` primitive and `useSettings`, while this
 * Rust port reads the latest `status_line_update` snapshot from the store and
 * passes it to `CustomStatusLine`, which renders it.
 *
 * The built-in statusline is always shown so operators always have a baseline
 * context bar, even when `status_line_update` events are not flowing. When the
 * backend has emitted a recent `status_line_update` with at least one non-empty
 * line and no error, the custom statusline renders below the built-in row so both
 * remain visible. The built-in row's error badge is used to indicate custom
 * statusline issues.
 */

interface Props {
  cwd: string
  model: string
  usage: Usage
  vimMode?: string
  viewMode?: ViewMode
}

export function StatusLine(props: Props) {
  const { customStatusLine } = useAppState()
  const showCustom = shouldRenderCustomStatusLine(customStatusLine)

  return (
    <box flexDirection="column">
      <BuiltinStatusLine {...props} />
      {showCustom && customStatusLine && (
        <CustomStatusLine snapshot={customStatusLine} />
      )}
    </box>
  )
}

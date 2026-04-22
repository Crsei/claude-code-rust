import React from 'react'
import type { ViewMode } from '../../keybindings.js'
import { useAppState } from '../../store/app-store.js'
import type { Usage } from '../../store/app-store.js'
import { BuiltinStatusLine } from './BuiltinStatusLine.js'
import { CustomStatusLine } from './CustomStatusLine.js'
import { shouldRenderCustomStatusLine } from './status-line-state.js'

/**
 * Orchestrator that composes the built-in statusline with the
 * optional user-configured custom statusline.
 *
 * The built-in statusline is always shown — the frontend derives it
 * directly from the Lite store so operators always have a baseline
 * context bar, even when `status_line_update` events aren't flowing.
 *
 * When the backend has recently emitted a `status_line_update` with
 * at least one non-empty line and no error, the custom statusline is
 * rendered *below* the built-in row so both stay visible. The error
 * badge on the built-in row acts as the status indicator for the
 * custom runner itself.
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

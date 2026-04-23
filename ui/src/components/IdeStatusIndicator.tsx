import React from 'react'
import { basename } from 'node:path'
import { useAppState } from '../store/app-store.js'
import type { IdeSelectionSnapshot } from '../store/app-state.js'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/IdeStatusIndicator.tsx`.
 *
 * Upstream reads `useIdeConnectionStatus(mcpClients)` to decide whether
 * an IDE integration is live, then renders "⧉ N lines selected" or
 * "⧉ In <filename>" depending on the `IDESelection` payload. cc-rust
 * wires the same two inputs through `state.ide`:
 *   - `connected` is toggled by `IdeEvent::connection_state_changed`
 *     (dispatched in `App.tsx`).
 *   - `selection` is dropped in by whatever MCP/IDE bridge ships the
 *     richer `{filePath, text, lineCount}` payload.
 *
 * Callers can still pass an explicit `selection` prop (e.g. the Lite
 * status-line builder) to override the store — useful when the
 * indicator needs to render a stale selection during a reconnect flap.
 */

type Props = {
  /** Override the store's selection snapshot. */
  selection?: IdeSelectionSnapshot | null
  /** Override the store's connection state. */
  forceConnected?: boolean
}

export function IdeStatusIndicator({ selection, forceConnected }: Props = {}) {
  const storeState = useAppState().ide
  const connected = forceConnected ?? storeState.connected
  const activeSelection = selection ?? storeState.selection

  if (!connected || !activeSelection) return null

  if (activeSelection.text && activeSelection.lineCount > 0) {
    return (
      <text fg={c.info}>
        {'\u2A9F '}
        {activeSelection.lineCount}{' '}
        {activeSelection.lineCount === 1 ? 'line' : 'lines'} selected
      </text>
    )
  }

  if (activeSelection.filePath) {
    return (
      <text fg={c.info}>
        {'\u2A9F '}In {basename(activeSelection.filePath)}
      </text>
    )
  }

  return null
}

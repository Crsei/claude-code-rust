import React from 'react'
import { c } from '../../theme.js'
import type { OrphanToolResultRenderItem } from '../../store/message-model.js'

/**
 * OpenTUI port of the orphan-result branch of upstream
 * `UserToolResultMessage` (the one taken when no matching `tool_use`
 * parent exists). Shown only when the pipeline sees a `tool_result`
 * with no corresponding `tool_use` — usually a replay artefact, a
 * cancelled call, or a background-task completion.
 *
 * Upstream doesn't render a dedicated glyph for this case — we use the
 * `⎿` continuation glyph so it reads as "result detached from its tool"
 * against the surrounding assistant / tool-use rows.
 */

type Props = {
  item: OrphanToolResultRenderItem
}

export function ToolResultOrphanMessage({ item }: Props) {
  const color = item.isError
    ? c.error
    : item.status === 'cancelled'
      ? c.warning
      : c.success
  const label = item.isError
    ? 'orphan error'
    : item.status === 'cancelled'
      ? 'orphan cancelled'
      : 'orphan result'

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box flexDirection="row" gap={1}>
        <box minWidth={2} flexShrink={0}>
          <text fg={color}>{'\u23BF'}</text>
        </box>
        <text fg={color}>
          <strong>{label}</strong>
        </text>
        <text fg={c.dim}>({item.toolUseId.slice(0, 8)})</text>
      </box>
      <box paddingLeft={3} width="100%">
        <text selectable fg={item.isError ? c.error : c.dim}>
          {item.outputSummary}
        </text>
      </box>
    </box>
  )
}

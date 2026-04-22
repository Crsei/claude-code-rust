import React from 'react'
import { c } from '../../theme.js'
import type { OrphanToolResultRenderItem } from '../../store/message-model.js'

/**
 * Lite-native port of the sample tree's orphan-result rendering from
 * `UserToolResultMessage` (the branch taken when there is no matching
 * `tool_use` parent). Shown only when the pipeline sees a `tool_result`
 * with no corresponding `tool_use` — usually a replay artifact or a
 * cancelled call.
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
  const label = item.isError ? 'ORPHAN ERROR' : 'ORPHAN RESULT'

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box gap={1}>
        <text fg={color}>
          <strong>[{label}]</strong>
        </text>
        <text fg={c.dim}>({item.toolUseId.slice(0, 8)})</text>
      </box>
      <box paddingLeft={2} width="100%">
        <text selectable fg={item.isError ? c.error : c.dim}>
          {item.outputSummary}
        </text>
      </box>
    </box>
  )
}

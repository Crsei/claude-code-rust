import React from 'react'
import { c } from '../theme.js'
import type { ToolGroupRenderItem } from '../store/message-model.js'

interface Props {
  item: ToolGroupRenderItem
}

const STATUS_COLORS = {
  pending: c.dim,
  running: c.info,
  success: c.success,
  error: c.error,
  cancelled: c.warning,
} as const

export function ToolGroup({ item }: Props) {
  const color = STATUS_COLORS[item.status]
  const latest = item.latestSummary || item.activities[item.activities.length - 1]?.inputSummary || ''

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box gap={1}>
        <text fg={color}>
          <strong>[GROUP]</strong>
        </text>
        <text fg={c.warning}>
          <strong>{item.label}</strong>
        </text>
        <text fg={c.dim}>{item.activities.length} calls</text>
      </box>
      <box paddingLeft={2} width="100%">
        <text fg={c.dim} selectable>
          {latest}
        </text>
      </box>
    </box>
  )
}

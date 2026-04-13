import React from 'react'
import type { ViewMode } from '../keybindings.js'
import { c } from '../theme.js'
import type { ToolActivityRenderItem } from '../store/message-model.js'

interface Props {
  item: ToolActivityRenderItem
  viewMode: ViewMode
}

const STATUS_STYLES = {
  pending: { label: 'PENDING', color: c.dim },
  running: { label: 'RUN', color: c.info },
  success: { label: 'OK', color: c.success },
  error: { label: 'ERROR', color: c.error },
  cancelled: { label: 'CANCELLED', color: c.warning },
} as const

export function ToolActivity({ item, viewMode }: Props) {
  const status = STATUS_STYLES[item.status]
  const detail = item.outputSummary
    ? `${item.inputSummary} -> ${item.outputSummary}`
    : item.inputSummary || '(no input summary)'

  if (viewMode === 'prompt') {
    return (
      <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
        <box gap={1} width="100%">
          <text fg={status.color}>
            <strong>[{status.label}]</strong>
          </text>
          <text fg={c.warning}>
            <strong>{item.name}</strong>
          </text>
          <text fg={item.isError ? c.error : c.dim} selectable>
            {detail}
          </text>
        </box>
      </box>
    )
  }

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box border={['left']} borderColor={status.color} paddingLeft={1} flexDirection="column">
        <box gap={1}>
          <text fg={status.color}>
            <strong>[{status.label}]</strong>
          </text>
          <text fg={c.warning}>
            <strong>{item.name}</strong>
          </text>
          <text fg={c.dim}>({item.toolUseId.slice(0, 8)})</text>
        </box>
        <box paddingLeft={2} flexDirection="column" width="100%">
          <text fg={c.dim}>Input</text>
          <text selectable>{item.inputSummary || '(no input summary)'}</text>
          <text fg={c.dim}>Result</text>
          <text selectable fg={item.isError ? c.error : c.text}>
            {item.outputSummary || '(waiting for result)'}
          </text>
        </box>
      </box>
    </box>
  )
}

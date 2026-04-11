import React from 'react'
import { c } from '../theme.js'

interface Props {
  output: string
  isError: boolean
  toolUseId: string
}

export function ToolResultBlock({ output, isError, toolUseId }: Props) {
  const maxLen = 2000
  const truncated = output.length > maxLen
  const displayText = truncated ? output.slice(0, maxLen) + '...' : output

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box gap={1}>
        <text>
          <strong><span fg={isError ? c.error : c.success}>
            {isError ? '✗' : '✓'} Result
          </span></strong>
        </text>
        <text fg={c.dim}>({toolUseId.slice(0, 8)})</text>
      </box>
      <box paddingLeft={2} width="100%">
        <text selectable fg={isError ? c.error : undefined}>{displayText}</text>
      </box>
      {truncated && (
        <box paddingLeft={2}>
          <text><em><span fg={c.dim}>[{output.length} chars total, truncated]</span></em></text>
        </box>
      )}
    </box>
  )
}

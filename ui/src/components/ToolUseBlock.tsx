import React, { useState } from 'react'
import { c } from '../theme.js'

interface Props {
  name: string
  input: any
  id: string
}

export function ToolUseBlock({ name, input, id }: Props) {
  const [expanded, setExpanded] = useState(false)

  const inputStr = typeof input === 'string' ? input : JSON.stringify(input, null, 2)
  const isLong = inputStr.length > 200

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box gap={1}>
        <text><strong><span fg={c.warning}>{'⚡'} {name}</span></strong></text>
        <text fg={c.dim}>({id.slice(0, 8)})</text>
      </box>
      <box paddingLeft={2} flexDirection="column" width="100%">
        {isLong && !expanded ? (
          <>
            <text fg={c.dim}>{inputStr.slice(0, 200)}...</text>
            <text><em><span fg={c.dim}>[{inputStr.length} chars, truncated]</span></em></text>
          </>
        ) : (
          <text fg={c.dim}>{inputStr}</text>
        )}
      </box>
    </box>
  )
}

import React, { useState } from 'react'
import { c } from '../theme.js'

interface Props {
  content: string
}

export function ThinkingBlock({ content }: Props) {
  const [expanded, setExpanded] = useState(false)

  const preview = content.length > 100 ? content.slice(0, 100) + '...' : content

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1}>
      <text>
        <em><span fg={c.dim}>
          {'💭'} Thinking {expanded ? '▼' : '▶'} {!expanded && `(${content.length} chars)`}
        </span></em>
      </text>
      <box paddingLeft={2}>
        <text><em><span fg={c.dim}>{expanded ? content : preview}</span></em></text>
      </box>
    </box>
  )
}

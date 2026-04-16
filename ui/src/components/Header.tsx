import React from 'react'
import type { ViewMode } from '../keybindings.js'
import { c } from '../theme.js'
import type { Usage } from '../store/app-store.js'
import { formatCost, formatTokens } from '../utils.js'

interface Props {
  cwd: string
  model: string
  usage: Usage
  vimMode?: string
  viewMode?: ViewMode
}

export function Header({ cwd, model, usage, vimMode, viewMode = 'prompt' }: Props) {
  const dir = cwd.replace(/\\/g, '/').split('/').pop() || cwd

  return (
    <box
      flexDirection="row"
      paddingX={1}
      border={['bottom']}
      borderStyle="single"
      borderColor="#45475A"
    >
      <text fg={c.accent}>{dir}</text>
      <text fg={c.dim}> | </text>
      {viewMode === 'transcript' && (
        <>
          <text fg={c.info}>
            <strong>[TRANSCRIPT]</strong>
          </text>
          <text fg={c.dim}> | </text>
        </>
      )}
      {vimMode && (
        <>
          <text>
            <strong>
              <span fg={c.warning}>[{vimMode}]</span>
            </strong>
          </text>
          <text fg={c.dim}> | </text>
        </>
      )}
      <text>{model}</text>
      <box flexGrow={1} />
      <text fg={c.dim}>Tokens: </text>
      <text>{formatTokens(usage.inputTokens + usage.outputTokens)}</text>
      <text fg={c.dim}> | Cost: </text>
      <text fg={c.success}>{formatCost(usage.costUsd)}</text>
    </box>
  )
}

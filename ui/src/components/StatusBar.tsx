import React from 'react'
import { c } from '../theme.js'
import type { Usage } from '../store/app-store.js'
import { formatCost, formatTokens } from '../utils.js'

export function StatusBar({ usage, model, vimMode }: { usage: Usage; model: string; vimMode?: string }) {
  return (
    <box paddingX={1} borderStyle="single" borderTop borderBottom={false} borderLeft={false} borderRight={false}>
      {vimMode && (
        <>
          <text><strong><span fg={c.warning}>[{vimMode}]</span></strong></text>
          <text><span fg={c.dim}> | </span></text>
        </>
      )}
      <text><span fg={c.dim}>Model: </span></text>
      <text>{model}</text>
      <box flexGrow={1} />
      <text><span fg={c.dim}>Tokens: </span></text>
      <text>{formatTokens(usage.inputTokens + usage.outputTokens)}</text>
      <text><span fg={c.dim}> | Cost: </span></text>
      <text fg={c.success}>{formatCost(usage.costUsd)}</text>
    </box>
  )
}

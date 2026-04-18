import React from 'react'
import type { ViewMode } from '../keybindings.js'
import { c } from '../theme.js'
import type { Usage } from '../store/app-store.js'
import { useAppState } from '../store/app-store.js'
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
  const { agentTree, subsystems, teams } = useAppState()

  // Count running agents across the tree
  const runningAgents = countRunning(agentTree)
  const activeTeams = Object.values(teams).filter(t => t.members.some(m => m.is_active)).length
  const connectedMcp = subsystems.mcp.filter(m => m.state === 'connected').length
  const runningLsp = subsystems.lsp.filter(l => l.state === 'running').length

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
      {runningAgents > 0 && (
        <>
          <text fg={c.dim}> | </text>
          <text fg="#A6E3A1">{runningAgents} agent{runningAgents > 1 ? 's' : ''}</text>
        </>
      )}
      {activeTeams > 0 && (
        <>
          <text fg={c.dim}> | </text>
          <text fg="#CBA6F7">{activeTeams} team{activeTeams > 1 ? 's' : ''}</text>
        </>
      )}
      {(connectedMcp > 0 || runningLsp > 0) && (
        <>
          <text fg={c.dim}> | </text>
          {runningLsp > 0 && <text fg="#89B4FA">LSP:{runningLsp}</text>}
          {runningLsp > 0 && connectedMcp > 0 && <text fg={c.dim}>/</text>}
          {connectedMcp > 0 && <text fg="#CBA6F7">MCP:{connectedMcp}</text>}
        </>
      )}
      <box flexGrow={1} />
      <text fg={c.dim}>Tokens: </text>
      <text>{formatTokens(usage.inputTokens + usage.outputTokens)}</text>
      <text fg={c.dim}> | Cost: </text>
      <text fg={c.success}>{formatCost(usage.costUsd)}</text>
    </box>
  )
}

function countRunning(nodes: Array<{ state: string; children: any[] }>): number {
  let n = 0
  for (const node of nodes) {
    if (node.state === 'running') n++
    n += countRunning(node.children)
  }
  return n
}

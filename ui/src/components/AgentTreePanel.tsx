import React from 'react'
import type { AgentNode } from '../ipc/protocol.js'
import type { AgentStreamState } from '../store/app-store.js'
import { useAppState } from '../store/app-store.js'

// ---------------------------------------------------------------------------
// State badge colors
// ---------------------------------------------------------------------------

const stateColor: Record<string, string> = {
  running: '#A6E3A1',
  completed: '#6C7086',
  error: '#F38BA8',
  aborted: '#FAB387',
}

const stateIcon: Record<string, string> = {
  running: '*',
  completed: '+',
  error: '!',
  aborted: 'x',
}

// ---------------------------------------------------------------------------
// Single agent node row
// ---------------------------------------------------------------------------

function AgentRow({ node, streams, depth = 0 }: {
  node: AgentNode
  streams: Record<string, AgentStreamState>
  depth?: number
}) {
  const indent = '  '.repeat(depth)
  const prefix = depth > 0 ? '|-- ' : ''
  const icon = stateIcon[node.state] ?? '?'
  const color = stateColor[node.state] ?? '#CDD6F4'

  const durationLabel = node.duration_ms != null
    ? ` ${(node.duration_ms / 1000).toFixed(1)}s`
    : ''

  const modelLabel = node.model
    ? ` (${node.model.replace(/^claude-/, '').replace(/-\d{8}$/, '')})`
    : ''

  const bgLabel = node.is_background ? ' [bg]' : ''
  const stream = streams[node.agent_id]
  const streamPreview = stream && node.state === 'running' && stream.text.length > 0
    ? ` -- ${stream.text.slice(-60).replace(/\n/g, ' ')}`
    : ''

  return (
    <>
      <text>
        <span fg="#585B70">{indent}{prefix}</span>
        <span fg={color}>[{icon}]</span>
        {' '}
        <span fg="#CDD6F4">{node.description}</span>
        <span fg="#6C7086">{modelLabel}{bgLabel}{durationLabel}</span>
        {streamPreview && <span fg="#585B70">{streamPreview}</span>}
      </text>
      {node.children.map(child => (
        <AgentRow
          key={child.agent_id}
          node={child}
          streams={streams}
          depth={depth + 1}
        />
      ))}
    </>
  )
}

// ---------------------------------------------------------------------------
// Panel
// ---------------------------------------------------------------------------

export function AgentTreePanel() {
  const { agentTree, agentStreams } = useAppState()

  if (agentTree.length === 0) return null

  const runningCount = countByState(agentTree, 'running')
  const title = runningCount > 0
    ? `Agents (${runningCount} running)`
    : `Agents (${agentTree.length})`

  return (
    <box
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor="#45475A"
      paddingX={1}
      title={title}
      titleAlignment="left"
    >
      {agentTree.map(root => (
        <AgentRow
          key={root.agent_id}
          node={root}
          streams={agentStreams}
        />
      ))}
    </box>
  )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function countByState(nodes: AgentNode[], state: string): number {
  let count = 0
  for (const n of nodes) {
    if (n.state === state) count++
    count += countByState(n.children, state)
  }
  return count
}

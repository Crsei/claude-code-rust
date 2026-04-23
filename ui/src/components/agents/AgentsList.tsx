import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import type { AgentSource } from './types.js'
import { getAgentSourceDisplayName } from './utils.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/AgentsList.tsx`.
 *
 * Grouped, scrollable list of agents. Upstream supports source-level
 * collapse and per-source editing hints; the Lite port groups entries
 * by `entry.source.kind`, renders a header + indented rows, and walks
 * the focused row with ↑/↓ / j/k. Enter triggers `onSelect`.
 */

type Props = {
  agents: AgentDefinitionEntry[]
  filter?: AgentSource
  onSelect: (agent: AgentDefinitionEntry) => void
  onCancel?: () => void
}

type Group = {
  source: AgentSource
  label: string
  entries: AgentDefinitionEntry[]
}

function entrySource(entry: AgentDefinitionEntry): AgentSource {
  const kind = entry.source?.kind
  if (kind === 'user') return 'userSettings'
  if (kind === 'project') return 'projectSettings'
  if (kind === 'builtin') return 'built-in'
  if (kind === 'plugin') return 'plugin'
  return 'userSettings'
}

export function AgentsList({ agents, filter = 'all', onSelect, onCancel }: Props) {
  const [focus, setFocus] = useState(0)

  const filtered = useMemo(() => {
    if (filter === 'all') return agents
    return agents.filter(a => entrySource(a) === filter)
  }, [agents, filter])

  const groups = useMemo<Group[]>(() => {
    const map = new Map<AgentSource, AgentDefinitionEntry[]>()
    for (const entry of filtered) {
      const src = entrySource(entry)
      if (!map.has(src)) map.set(src, [])
      map.get(src)!.push(entry)
    }
    const order: AgentSource[] = [
      'built-in',
      'userSettings',
      'projectSettings',
      'localSettings',
      'policySettings',
      'flagSettings',
      'plugin',
    ]
    return order
      .filter(src => map.has(src))
      .map(src => ({
        source: src,
        label: getAgentSourceDisplayName(src),
        entries: map.get(src)!.slice().sort((a, b) => a.name.localeCompare(b.name)),
      }))
  }, [filtered])

  const flat = useMemo(() => {
    const out: AgentDefinitionEntry[] = []
    for (const g of groups) out.push(...g.entries)
    return out
  }, [groups])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'up' || key === 'k') {
      setFocus(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setFocus(idx => Math.min(flat.length - 1, idx + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const entry = flat[focus]
      if (entry) onSelect(entry)
    }
  })

  if (flat.length === 0) {
    return (
      <box paddingLeft={2}>
        <text fg={c.dim}>No agents in this scope.</text>
      </box>
    )
  }

  let globalIndex = 0
  return (
    <box flexDirection="column">
      {groups.map(group => (
        <box key={group.source} flexDirection="column" marginBottom={1}>
          <box paddingLeft={1}>
            <strong><text fg={c.accent}>{group.label}</text></strong>
          </box>
          {group.entries.map(entry => {
            const thisIndex = globalIndex++
            const isFocused = thisIndex === focus
            return (
              <box key={`${group.source}:${entry.name}`} flexDirection="row" paddingLeft={2} gap={1}>
                <text fg={isFocused ? c.accent : c.dim}>
                  {isFocused ? '\u276F' : ' '}
                </text>
                {isFocused ? (
                  <strong><text fg={c.textBright}>{entry.name}</text></strong>
                ) : (
                  <text>{entry.name}</text>
                )}
                {entry.description && (
                  <text fg={c.dim}>— {truncate(entry.description, 60)}</text>
                )}
              </box>
            )
          })}
        </box>
      ))}
      <box marginTop={1}>
        <text fg={c.dim}>\u2191/\u2193 select · Enter view · Esc close</text>
      </box>
    </box>
  )
}

function truncate(s: string, n: number): string {
  return s.length <= n ? s : s.slice(0, n - 1) + '\u2026'
}

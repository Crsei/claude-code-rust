import React from 'react'
import type { AgentDefinitionEntry } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import { sourceColor, sourceLabel } from './constants.js'

/**
 * Read-only list rendering for the agents dialog. Keyboard handling lives
 * one level up (`AgentsDialog`) so we can share arrow-key logic between the
 * list, detail, and form views.
 *
 * Agents are grouped by source with a visible section header. Within each
 * section entries are sorted alphabetically by the parent — we render them
 * in the order given.
 */

export interface AgentsListViewProps {
  entries: AgentDefinitionEntry[]
  selectedIndex: number
  createNewSelected: boolean
}

export function AgentsListView({
  entries,
  selectedIndex,
  createNewSelected,
}: AgentsListViewProps) {
  if (entries.length === 0 && !createNewSelected) {
    return (
      <box flexDirection="column" paddingY={1}>
        <text><span fg={c.dim}>No agents discovered yet.</span></text>
      </box>
    )
  }

  // Render "create new" first, then a single flat list keyed by overall
  // index so keyboard navigation stays predictable.
  return (
    <box flexDirection="column">
      <box flexDirection="row">
        <text>
          <span fg={createNewSelected ? c.accent : c.dim}>
            {createNewSelected ? '▸ ' : '  '}
          </span>
          <span fg={createNewSelected ? c.accent : c.text}>Create new agent…</span>
        </text>
      </box>

      {renderGroupedList(entries, selectedIndex, createNewSelected)}
    </box>
  )
}

function renderGroupedList(
  entries: AgentDefinitionEntry[],
  selectedIndex: number,
  createNewSelected: boolean,
): React.ReactNode {
  const groups: Array<{ label: string; start: number; items: AgentDefinitionEntry[] }> = []

  let cursor = 0
  const orderedKinds: ReadonlyArray<'user' | 'project' | 'plugin' | 'builtin'> = [
    'user',
    'project',
    'plugin',
    'builtin',
  ]
  const byKind: Record<string, AgentDefinitionEntry[]> = {}
  for (const entry of entries) {
    const key = entry.source.kind === 'plugin' ? `plugin:${entry.source.id}` : entry.source.kind
    ;(byKind[key] ??= []).push(entry)
  }

  for (const kind of orderedKinds) {
    const keys = Object.keys(byKind).filter(k => k === kind || k.startsWith(`${kind}:`))
    for (const k of keys) {
      const items = byKind[k]!
      const label =
        k === 'user'
          ? 'User agents'
          : k === 'project'
            ? 'Project agents'
            : k === 'builtin'
              ? 'Built-in (read-only)'
              : `Plugin ${k.replace(/^plugin:/, '')} (read-only)`
      groups.push({ label, start: cursor, items })
      cursor += items.length
    }
  }

  return (
    <box flexDirection="column">
      {groups.map(group => (
        <box key={group.label} flexDirection="column" marginTop={1}>
          <text>
            <strong><span fg={c.info}>{group.label}</span></strong>
          </text>
          {group.items.map((entry, localIdx) => {
            const globalIdx = group.start + localIdx
            const isSelected = !createNewSelected && globalIdx === selectedIndex
            const color = sourceColor(entry.source)
            return (
              <box key={`${entry.source.kind}-${entry.name}`} flexDirection="row">
                <text>
                  <span fg={isSelected ? c.accent : c.dim}>
                    {isSelected ? '▸ ' : '  '}
                  </span>
                  <span fg={isSelected ? c.textBright : c.text}>{entry.name}</span>
                  <span fg={c.dim}>{' · '}</span>
                  <span fg={color}>{sourceLabel(entry.source)}</span>
                  {entry.model ? (
                    <>
                      <span fg={c.dim}>{' · '}</span>
                      <span fg={c.dim}>{entry.model}</span>
                    </>
                  ) : null}
                  {entry.description ? (
                    <>
                      <span fg={c.dim}>{'  — '}</span>
                      <span fg={c.dim}>{truncate(entry.description, 70)}</span>
                    </>
                  ) : null}
                </text>
              </box>
            )
          })}
        </box>
      ))}
    </box>
  )
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s
  return s.slice(0, max - 1) + '…'
}

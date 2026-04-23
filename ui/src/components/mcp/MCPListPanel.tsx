import React, { useCallback, useMemo } from 'react'
import { useKeyboard } from '@opentui/react'
import type { ConfigScope } from '../../ipc/protocol.js'
import { c } from '../../theme.js'
import type { ServerInfo } from './types.js'
import {
  describeMcpConfigFilePath,
  plural,
  serverDisplayState,
} from './utils.js'

/**
 * Server list with scope grouping + keyboard navigation.
 *
 * Renders each non-empty scope as its own section: Project → Local →
 * User → Enterprise → Built-in. Inside each section, servers sort
 * alphabetically.
 *
 * (cc-rust's `ConfigScope` enum has `user` / `project` / `plugin` / `ide`;
 * "local"/"enterprise" are upstream-only — the rendering code below
 * only lists buckets we actually populate.)
 */

type SectionKey = 'project' | 'user' | 'plugin' | 'ide'

const SECTION_ORDER: SectionKey[] = ['project', 'user', 'plugin', 'ide']

function sectionKeyOf(scope: ConfigScope): SectionKey {
  return scope.kind
}

function sectionHeading(scope: ConfigScope): { label: string; path?: string } {
  switch (scope.kind) {
    case 'project':
      return { label: 'Project MCPs', path: describeMcpConfigFilePath(scope) }
    case 'user':
      return { label: 'User MCPs', path: describeMcpConfigFilePath(scope) }
    case 'plugin':
      return {
        label: 'Plugin MCPs',
        path: scope.id ? `plugin:${scope.id}` : 'plugin-contributed',
      }
    case 'ide':
      return {
        label: 'IDE MCPs',
        path: scope.id ? `ide:${scope.id}` : 'ide-contributed',
      }
  }
}

type Props = {
  servers: ServerInfo[]
  selectedIndex: number
  onSelect: (server: ServerInfo) => void
  onCancel: () => void
  onHover: (index: number) => void
  lastError?: string | null
  lastMessage?: string | null
}

export function MCPListPanel({
  servers,
  selectedIndex,
  onSelect,
  onCancel,
  onHover,
  lastError,
  lastMessage,
}: Props) {
  const ordered = useMemo(() => groupServersByScope(servers), [servers])
  const flat = useMemo(
    () => ordered.flatMap(g => g.items),
    [ordered],
  )
  const safeIndex =
    flat.length === 0 ? 0 : Math.max(0, Math.min(selectedIndex, flat.length - 1))

  const applyDelta = useCallback(
    (delta: number) => {
      if (flat.length === 0) return
      const next = (safeIndex + delta + flat.length) % flat.length
      onHover(next)
    },
    [flat.length, onHover, safeIndex],
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'escape') {
      onCancel()
      return
    }
    if (event.name === 'up' || event.name === 'k') {
      applyDelta(-1)
      return
    }
    if (event.name === 'down' || event.name === 'j' || event.name === 'tab') {
      applyDelta(1)
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const target = flat[safeIndex]
      if (target) onSelect(target)
    }
  })

  if (servers.length === 0) {
    return (
      <box flexDirection="column" paddingX={1} paddingY={1}>
        <text>
          <span fg={c.dim}>
            No MCP servers configured. Edit ~/.cc-rust/settings.json or run `/mcp
            add &lt;name&gt;` to add one.
          </span>
        </text>
      </box>
    )
  }

  const total = servers.length

  return (
    <box flexDirection="column">
      <box paddingX={1} marginBottom={1}>
        <text>
          <strong>Manage MCP servers</strong>
          <span fg={c.dim}>{`  ${total} ${plural(total, 'server')}`}</span>
        </text>
      </box>

      {ordered.map(group => (
        <box key={group.key} flexDirection="column" marginBottom={1}>
          <box paddingX={2}>
            <text>
              <strong>{group.heading.label}</strong>
              {group.heading.path && (
                <span fg={c.dim}>{` (${group.heading.path})`}</span>
              )}
            </text>
          </box>
          {group.items.map((info, idxInGroup) => {
            const globalIndex =
              group.offset + idxInGroup
            const isSelected = globalIndex === safeIndex
            return (
              <ServerRow
                key={`${info.scope.kind}-${info.name}-${group.offset}-${idxInGroup}`}
                info={info}
                isSelected={isSelected}
              />
            )
          })}
        </box>
      ))}

      <box paddingX={1} flexDirection="column">
        {lastError && (
          <text>
            <span fg={c.error}>{lastError}</span>
          </text>
        )}
        {!lastError && lastMessage && (
          <text>
            <span fg={c.success}>{lastMessage}</span>
          </text>
        )}
        <text>
          <em>
            <span fg={c.dim}>
              ↑↓ navigate · Enter select · Esc close
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}

function ServerRow({
  info,
  isSelected,
}: {
  info: ServerInfo
  isSelected: boolean
}) {
  const state = serverDisplayState(info)
  const { glyph, color, label } = renderState(state)
  const toolsCount = info.status?.tools_count ?? info.tools.length
  const resourcesCount = info.status?.resources_count ?? info.resources.length

  return (
    <box flexDirection="row">
      <text>
        <span fg={isSelected ? c.accent : c.dim}>
          {isSelected ? '▸ ' : '  '}
        </span>
        <span fg={isSelected ? c.textBright : c.text}>{info.name}</span>
        <span fg={c.dim}> · </span>
        <span fg={color}>{glyph}</span>
        <span fg={c.dim}>{' '}</span>
        <span fg={c.dim}>{label}</span>
        <span fg={c.dim}>{`  [${info.transport}]`}</span>
        {toolsCount > 0 && (
          <span fg={c.dim}>
            {`  · ${toolsCount} ${plural(toolsCount, 'tool')}`}
          </span>
        )}
        {resourcesCount > 0 && (
          <span fg={c.dim}>
            {`  · ${resourcesCount} ${plural(resourcesCount, 'resource')}`}
          </span>
        )}
      </text>
    </box>
  )
}

function renderState(
  state: 'disabled' | 'connected' | 'pending' | 'failed' | 'unknown',
): { glyph: string; color: string; label: string } {
  switch (state) {
    case 'connected':
      return { glyph: '✔', color: c.success, label: 'connected' }
    case 'pending':
      return { glyph: '○', color: c.dim, label: 'connecting…' }
    case 'failed':
      return { glyph: '✖', color: c.error, label: 'failed' }
    case 'disabled':
      return { glyph: '○', color: c.dim, label: 'disabled' }
    default:
      return { glyph: '·', color: c.dim, label: 'unknown' }
  }
}

interface GroupBucket {
  key: string
  heading: { label: string; path?: string }
  items: ServerInfo[]
  offset: number
}

function groupServersByScope(servers: ServerInfo[]): GroupBucket[] {
  const byKey = new Map<string, ServerInfo[]>()
  for (const s of servers) {
    const bucketKey =
      s.scope.kind === 'plugin' || s.scope.kind === 'ide'
        ? `${s.scope.kind}:${s.scope.id ?? ''}`
        : s.scope.kind
    if (!byKey.has(bucketKey)) byKey.set(bucketKey, [])
    byKey.get(bucketKey)!.push(s)
  }

  const groups: GroupBucket[] = []
  let offset = 0
  for (const kind of SECTION_ORDER) {
    const keys = [...byKey.keys()].filter(
      k => k === kind || k.startsWith(`${kind}:`),
    )
    keys.sort()
    for (const k of keys) {
      const items = byKey.get(k) ?? []
      items.sort((a, b) => a.name.localeCompare(b.name))
      if (items.length === 0) continue
      groups.push({
        key: k,
        heading: sectionHeading(items[0]!.scope),
        items,
        offset,
      })
      offset += items.length
    }
  }
  return groups
}

// Exported for tests / deep linkers. Not used by default render.
export { groupServersByScope as __groupServersByScope, sectionKeyOf as __sectionKeyOf }

import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/ToolSelector.tsx`.
 *
 * Upstream's 477-line component carries full tool-permission context,
 * MCP-tool merging, group-collapse state, and inline `?` help.
 *
 * The Lite port keeps the user-facing contract — a scrollable list of
 * tools with `[ ] / [x]` gutters, grouped by category — plus the three
 * meta-selections upstream supports:
 *
 *  - `all`     (`selected === undefined`)
 *  - `none`    (empty array)
 *  - explicit subset
 *
 * Groups are collapsed to section headers but don't re-implement the
 * tree widget — flat list with "(group)" prefix is sufficient for the
 * Lite UX today. The caller passes the tool catalog so this component
 * doesn't need to resolve the Tool registry itself.
 */

export type ToolSpec = {
  name: string
  group?: string
  description?: string
  /** When true, the tool is always selectable even if `selected === undefined`. */
  alwaysOn?: boolean
}

type Props = {
  tools: ToolSpec[]
  /** `undefined` = "all tools", `[]` = "no tools", otherwise an explicit subset. */
  selected: string[] | undefined
  onComplete: (selected: string[] | undefined) => void
  onCancel?: () => void
}

type Mode = 'all' | 'none' | 'subset'

function initialMode(selected: string[] | undefined): Mode {
  if (selected === undefined) return 'all'
  if (selected.length === 0) return 'none'
  return 'subset'
}

export function ToolSelector({ tools, selected, onComplete, onCancel }: Props) {
  const [mode, setMode] = useState<Mode>(() => initialMode(selected))
  const [focus, setFocus] = useState(0)
  const [explicit, setExplicit] = useState<Set<string>>(
    () => new Set(selected ?? []),
  )

  const grouped = useMemo(() => {
    const grouped: ToolSpec[] = []
    const withGroup = tools.filter(t => t.group)
    const groups = new Map<string, ToolSpec[]>()
    for (const tool of withGroup) {
      const key = tool.group ?? ''
      if (!groups.has(key)) groups.set(key, [])
      groups.get(key)!.push(tool)
    }
    for (const [group, members] of groups) {
      grouped.push({ name: `\u2014 ${group} \u2014`, group, description: '' })
      grouped.push(...members)
    }
    const ungrouped = tools.filter(t => !t.group)
    if (ungrouped.length > 0) {
      if (grouped.length > 0) grouped.push({ name: '\u2014 other \u2014', description: '' })
      grouped.push(...ungrouped)
    }
    return grouped
  }, [tools])

  const rows = useMemo(() => {
    const meta: Array<{
      kind: 'meta'
      key: string
      label: string
      mode: Mode
    }> = [
      { kind: 'meta', key: 'all', label: 'All tools (inherit)', mode: 'all' },
      { kind: 'meta', key: 'none', label: 'No tools', mode: 'none' },
      { kind: 'meta', key: 'subset', label: 'Custom subset', mode: 'subset' },
    ]
    const toolRows = grouped.map(t => ({
      kind: 'tool' as const,
      key: t.name,
      tool: t,
    }))
    return [...meta, ...toolRows]
  }, [grouped])

  function toggleFocused() {
    const row = rows[focus]
    if (!row) return
    if (row.kind === 'meta') {
      setMode(row.mode)
      return
    }
    if (row.tool.group && row.tool.name.startsWith('\u2014')) return
    if (mode !== 'subset') setMode('subset')
    setExplicit(prev => {
      const next = new Set(prev)
      if (next.has(row.tool.name)) next.delete(row.tool.name)
      else next.add(row.tool.name)
      return next
    })
  }

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'return' || name === 'enter') {
      if (mode === 'all') onComplete(undefined)
      else if (mode === 'none') onComplete([])
      else onComplete(Array.from(explicit))
      return
    }
    if (name === 'up' || key === 'k') {
      setFocus(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setFocus(idx => Math.min(rows.length - 1, idx + 1))
      return
    }
    if (key === ' ' || name === 'tab') {
      toggleFocused()
    }
  })

  return (
    <box flexDirection="column">
      {rows.map((row, i) => {
        const isFocused = i === focus
        if (row.kind === 'meta') {
          const active = mode === row.mode
          return (
            <box key={row.key} flexDirection="row" gap={1}>
              <text fg={isFocused ? c.accent : c.dim}>
                {isFocused ? '\u276F' : ' '}
              </text>
              <text fg={active ? c.success : c.dim}>
                {active ? '\u25CF' : '\u25CB'}
              </text>
              {isFocused ? (
                <strong><text fg={c.textBright}>{row.label}</text></strong>
              ) : (
                <text>{row.label}</text>
              )}
            </box>
          )
        }
        if (row.tool.name.startsWith('\u2014')) {
          return (
            <box key={row.key} paddingLeft={2}>
              <text fg={c.dim}>{row.tool.name}</text>
            </box>
          )
        }
        const checked = mode === 'all' || (mode === 'subset' && explicit.has(row.tool.name))
        return (
          <box key={row.key} flexDirection="row" gap={1} paddingLeft={2}>
            <text fg={isFocused ? c.accent : c.dim}>
              {isFocused ? '\u276F' : ' '}
            </text>
            <text fg={checked ? c.success : c.dim}>
              {checked ? '[\u2713]' : '[ ]'}
            </text>
            {isFocused ? (
              <strong><text fg={c.textBright}>{row.tool.name}</text></strong>
            ) : (
              <text>{row.tool.name}</text>
            )}
            {row.tool.description && (
              <text fg={c.dim}>— {row.tool.description}</text>
            )}
          </box>
        )
      })}
      <box marginTop={1}>
        <text fg={c.dim}>
          Space toggle · Enter confirm · Esc cancel
        </text>
      </box>
    </box>
  )
}

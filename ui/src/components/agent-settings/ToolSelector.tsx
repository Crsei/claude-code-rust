import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../../ipc/context.js'
import type { AgentToolInfo } from '../../ipc/protocol.js'
import { useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'

/**
 * Bucket-based tool picker mirroring upstream `ToolSelector.tsx`. Shows:
 *  * a Continue button at the top,
 *  * an "All tools" master toggle,
 *  * one bucket per category (read-only, edit, execution, mcp, other),
 *  * an "Advanced options" disclosure that exposes the individual tools,
 *    MCP servers, and their members.
 *
 * `initialTools` uses upstream's semantic: `undefined` = "all tools";
 * anything else = explicit allow-list. On submit we convert the internal
 * state back to `undefined` when everything is selected to keep on-disk
 * frontmatter clean.
 */

interface Props {
  initialTools: string[] | undefined
  onComplete: (tools: string[] | undefined) => void
  onCancel: () => void
}

interface Bucket {
  id: string
  label: string
  tools: AgentToolInfo[]
}

export function ToolSelector({ initialTools, onComplete, onCancel }: Props) {
  const backend = useBackend()
  const { agentSettings } = useAppState()
  const tools = agentSettings.availableTools

  // Fetch the tool list when the selector mounts if we don't have one.
  useEffect(() => {
    if (tools.length === 0) {
      backend.send({
        type: 'agent_settings_command',
        command: { kind: 'query_tools' },
      })
    }
  }, [backend, tools.length])

  const allNames = useMemo(() => tools.map(t => t.name), [tools])
  const allSelected = initialTools === undefined || initialTools.includes('*')
  const expanded = allSelected ? allNames : initialTools ?? []

  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(expanded),
  )
  const [focus, setFocus] = useState(0)
  const [showIndividual, setShowIndividual] = useState(false)

  // Buckets match the upstream labels.
  const buckets: Bucket[] = useMemo(() => {
    const by: Record<string, AgentToolInfo[]> = {
      read_only: [],
      edit: [],
      execution: [],
      mcp: [],
      other: [],
    }
    for (const t of tools) (by[t.category] ??= []).push(t)
    return [
      { id: 'readOnly', label: 'Read-only tools', tools: by.read_only! },
      { id: 'edit', label: 'Edit tools', tools: by.edit! },
      { id: 'execution', label: 'Execution tools', tools: by.execution! },
      { id: 'mcp', label: 'MCP tools', tools: by.mcp! },
      { id: 'other', label: 'Other tools', tools: by.other! },
    ].filter(b => b.tools.length > 0)
  }, [tools])

  const toggleMany = useCallback((names: string[], select: boolean) => {
    setSelected(cur => {
      const next = new Set(cur)
      for (const n of names) {
        if (select) next.add(n)
        else next.delete(n)
      }
      return next
    })
  }, [])

  const toggleOne = useCallback((name: string) => {
    setSelected(cur => {
      const next = new Set(cur)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }, [])

  // Build a flat navigable list: Continue, All tools, one-per-bucket, toggle,
  // then (if expanded) MCP-servers + individual tools.
  type Item =
    | { kind: 'continue' }
    | { kind: 'all' }
    | { kind: 'bucket'; bucket: Bucket }
    | { kind: 'toggle' }
    | { kind: 'header'; label: string }
    | { kind: 'tool'; tool: AgentToolInfo }

  const items: Item[] = useMemo(() => {
    const base: Item[] = [{ kind: 'continue' }, { kind: 'all' }]
    for (const b of buckets) base.push({ kind: 'bucket', bucket: b })
    base.push({ kind: 'toggle' })
    if (showIndividual) {
      base.push({ kind: 'header', label: 'Individual tools' })
      for (const t of tools) base.push({ kind: 'tool', tool: t })
    }
    return base
  }, [buckets, showIndividual, tools])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'escape') {
      onCancel()
      return
    }
    if (event.name === 'up') {
      setFocus(i => {
        let next = i - 1
        while (next >= 0 && items[next]?.kind === 'header') next--
        return Math.max(0, next)
      })
      return
    }
    if (event.name === 'down' || event.name === 'tab') {
      setFocus(i => {
        let next = i + 1
        while (next < items.length && items[next]?.kind === 'header') next++
        return Math.min(items.length - 1, next)
      })
      return
    }
    if (event.name === 'return' || event.name === 'enter') {
      const item = items[focus]
      if (!item) return
      if (item.kind === 'continue') {
        const areAll =
          allNames.length > 0 && allNames.every(n => selected.has(n))
        onComplete(areAll ? undefined : Array.from(selected))
      } else if (item.kind === 'all') {
        const areAll =
          allNames.length > 0 && allNames.every(n => selected.has(n))
        toggleMany(allNames, !areAll)
      } else if (item.kind === 'bucket') {
        const names = item.bucket.tools.map(t => t.name)
        const areAll = names.every(n => selected.has(n))
        toggleMany(names, !areAll)
      } else if (item.kind === 'toggle') {
        setShowIndividual(s => !s)
      } else if (item.kind === 'tool') {
        toggleOne(item.tool.name)
      }
    }
  })

  const areAllSelected =
    allNames.length > 0 && allNames.every(n => selected.has(n))

  return (
    <box flexDirection="column" flexGrow={1}>
      {items.map((item, i) => {
        const focused = i === focus
        const prefix = focused ? '▸ ' : '  '
        if (item.kind === 'continue') {
          return (
            <text key="continue">
              <span fg={focused ? c.accent : c.dim}>{prefix}</span>
              <span fg={focused ? c.textBright : c.text}>[ Continue ]</span>
            </text>
          )
        }
        if (item.kind === 'all') {
          return (
            <text key="all">
              <span fg={focused ? c.accent : c.dim}>{prefix}</span>
              <span>{areAllSelected ? '[x]' : '[ ]'}</span>
              <span fg={focused ? c.textBright : c.text}> All tools</span>
            </text>
          )
        }
        if (item.kind === 'bucket') {
          const names = item.bucket.tools.map(t => t.name)
          const full = names.every(n => selected.has(n))
          const partial = !full && names.some(n => selected.has(n))
          const mark = full ? '[x]' : partial ? '[~]' : '[ ]'
          return (
            <text key={item.bucket.id}>
              <span fg={focused ? c.accent : c.dim}>{prefix}</span>
              <span>{mark}</span>
              <span fg={focused ? c.textBright : c.text}>
                {' '}{item.bucket.label}
                <span fg={c.dim}>
                  {' '}({item.bucket.tools.length} tool
                  {item.bucket.tools.length === 1 ? '' : 's'})
                </span>
              </span>
            </text>
          )
        }
        if (item.kind === 'toggle') {
          return (
            <text key="toggle">
              <span fg={focused ? c.accent : c.dim}>{prefix}</span>
              <span fg={focused ? c.textBright : c.text}>
                [ {showIndividual ? 'Hide' : 'Show'} advanced options ]
              </span>
            </text>
          )
        }
        if (item.kind === 'header') {
          return (
            <text key={`h-${item.label}`}>
              <span fg={c.dim}>{item.label}:</span>
            </text>
          )
        }
        const on = selected.has(item.tool.name)
        return (
          <text key={item.tool.name}>
            <span fg={focused ? c.accent : c.dim}>{prefix}</span>
            <span>{on ? '[x]' : '[ ]'}</span>
            <span fg={focused ? c.textBright : c.text}>{' '}{item.tool.name}</span>
            {item.tool.mcp_server ? (
              <span fg={c.dim}>{' '}({item.tool.mcp_server})</span>
            ) : null}
          </text>
        )
      })}
      <box marginTop={1}>
        <text>
          <span fg={c.dim}>
            {areAllSelected
              ? 'All tools selected'
              : `${selected.size} / ${allNames.length} tools selected`}
          </span>
        </text>
      </box>
    </box>
  )
}

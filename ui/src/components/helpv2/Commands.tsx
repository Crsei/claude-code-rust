import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import { truncate } from '../../utils.js'

/**
 * Scrollable list of commands used inside the HelpV2 dialog's Commands /
 * Custom-commands tabs.
 *
 * OpenTUI-native port of the upstream `helpv2/Commands`
 * (`ui/examples/upstream-patterns/src/components/helpv2/Commands.tsx`).
 * The upstream component rendered through Ink's `<Select>` with
 * `layout="compact-vertical"`. The Lite port implements the same
 * compact two-line layout (`/name` + truncated description) with
 * selection via `useKeyboard`.
 */

export type HelpCommandEntry = {
  name: string
  description: string
  /** Optional origin label (e.g. "project", "user"). Appended to desc. */
  source?: string
}

type Props = {
  commands: HelpCommandEntry[]
  maxHeight: number
  columns: number
  title: string
  emptyMessage?: string
  onCancel: () => void
  /** Set when the tab header is focused so the list does not intercept keys. */
  isDisabled?: boolean
  /** Called when the user presses Up on the first item so the parent can
   *  hand focus back to the tab header. */
  onUpFromFirstItem?: () => void
}

function formatDescriptionWithSource(entry: HelpCommandEntry): string {
  if (!entry.source) return entry.description
  return `${entry.description} (${entry.source})`
}

export function Commands({
  commands,
  maxHeight,
  columns,
  title,
  emptyMessage,
  onCancel,
  isDisabled = false,
  onUpFromFirstItem,
}: Props) {
  const maxWidth = Math.max(1, columns - 10)
  const visibleCount = Math.max(1, Math.floor((maxHeight - 10) / 2))

  const options = useMemo(() => {
    const seen = new Set<string>()
    return commands
      .filter(cmd => {
        if (seen.has(cmd.name)) return false
        seen.add(cmd.name)
        return true
      })
      .sort((a, b) => a.name.localeCompare(b.name))
      .map(cmd => ({
        label: `/${cmd.name}`,
        value: cmd.name,
        description: truncate(formatDescriptionWithSource(cmd), maxWidth),
      }))
  }, [commands, maxWidth])

  const [cursor, setCursor] = useState(0)
  const [windowStart, setWindowStart] = useState(0)

  useKeyboard(event => {
    if (isDisabled || event.eventType === 'release') return
    const name = event.name
    const input = event.sequence ?? name ?? ''
    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'up' || input === 'k') {
      if (cursor === 0) {
        onUpFromFirstItem?.()
        return
      }
      const next = cursor - 1
      setCursor(next)
      if (next < windowStart) setWindowStart(next)
      return
    }
    if (name === 'down' || input === 'j') {
      if (cursor === options.length - 1) return
      const next = cursor + 1
      setCursor(next)
      if (next >= windowStart + visibleCount) setWindowStart(next - visibleCount + 1)
    }
  })

  if (options.length === 0 && emptyMessage) {
    return (
      <box flexDirection="column" paddingY={1}>
        <text fg={c.dim}>{emptyMessage}</text>
      </box>
    )
  }

  const visible = options.slice(windowStart, windowStart + visibleCount)
  const hasAbove = windowStart > 0
  const hasBelow = windowStart + visibleCount < options.length

  return (
    <box flexDirection="column" paddingY={1}>
      <text>{title}</text>
      <box marginTop={1} flexDirection="column">
        {hasAbove && <text fg={c.dim}>{`\u25B4 ${windowStart} more\u2026`}</text>}
        {visible.map((opt, i) => {
          const index = windowStart + i
          const isSelected = index === cursor
          return (
            <box key={opt.value} flexDirection="column">
              <text
                fg={isSelected ? c.bg : c.accent}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` ${opt.label} `}</strong>
              </text>
              <text fg={c.dim}>{`   ${opt.description}`}</text>
            </box>
          )
        })}
        {hasBelow && (
          <text fg={c.dim}>{`\u25BE ${options.length - (windowStart + visibleCount)} more\u2026`}</text>
        )}
      </box>
    </box>
  )
}

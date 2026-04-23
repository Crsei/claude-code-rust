import React, { type ReactNode } from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `Tabs` — a horizontal row of tab
 * labels with an active highlight. The active tab is rendered in bold
 * + accent; inactive tabs dim. Consumers are expected to own state and
 * pass `activeId`.
 */

export type TabItem = {
  id: string
  label: string
  /** Optional trailing badge (e.g. "3" for unread count). */
  badge?: ReactNode
}

type Props = {
  items: TabItem[]
  activeId: string
  onChange?: (id: string) => void
  /** When true, an underline is drawn beneath the active tab. */
  underline?: boolean
}

export function Tabs({ items, activeId, underline = true }: Props) {
  return (
    <box flexDirection="column">
      <box flexDirection="row" gap={2}>
        {items.map(item => {
          const active = item.id === activeId
          return (
            <box key={item.id} flexDirection="row" gap={1}>
              {active ? (
                <strong>
                  <text fg={c.accent}>{item.label}</text>
                </strong>
              ) : (
                <text fg={c.dim}>{item.label}</text>
              )}
              {item.badge && <text fg={c.dim}>{item.badge}</text>}
            </box>
          )
        })}
      </box>
      {underline && (
        <box flexDirection="row" gap={2}>
          {items.map(item => {
            const active = item.id === activeId
            const width = item.label.length
            return (
              <text key={item.id} fg={active ? c.accent : c.dim}>
                {(active ? '\u2500' : ' ').repeat(width)}
              </text>
            )
          })}
        </box>
      )}
    </box>
  )
}

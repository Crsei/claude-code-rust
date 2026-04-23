import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import type { HookEvent, HookEventMetadata } from './types.js'
import { plural } from './types.js'

/**
 * Entry view for `/hooks` \u2014 pick a hook event to drill into.
 *
 * OpenTUI-native port of the upstream `hooks/SelectEventMode`
 * (`ui/examples/upstream-patterns/src/components/hooks/SelectEventMode.tsx`).
 * The menu is read-only: selecting an event lets you browse its
 * configured hooks but not modify them.
 */

const DOCS_URL = 'https://docs.claude.com/en/docs/claude-code/hooks'

type Props = {
  hookEventMetadata: Record<HookEvent, HookEventMetadata>
  hooksByEvent: Partial<Record<HookEvent, number>>
  totalHooksCount: number
  restrictedByPolicy: boolean
  onSelectEvent: (event: HookEvent) => void
  onCancel: () => void
}

export function SelectEventMode({
  hookEventMetadata,
  hooksByEvent,
  totalHooksCount,
  restrictedByPolicy,
  onSelectEvent,
  onCancel,
}: Props) {
  const events = Object.keys(hookEventMetadata) as HookEvent[]
  const [cursor, setCursor] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = event.sequence ?? name ?? ''
    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'up' || input === 'k') {
      setCursor(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setCursor(prev => Math.min(events.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const selected = events[cursor]
      if (selected) onSelectEvent(selected)
    }
  })

  const subtitle = `${totalHooksCount} ${plural(totalHooksCount, 'hook')} configured`

  return (
    <box
      position="absolute"
      top={2}
      left={2}
      right={2}
      bottom={2}
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor={c.warning}
      title={`Hooks \u00B7 ${subtitle}`}
      titleAlignment="left"
      paddingX={2}
      paddingY={1}
    >
      {restrictedByPolicy && (
        <box flexDirection="column" marginBottom={1}>
          <text fg={c.warning}>\u2139 Hooks Restricted by Policy</text>
          <text fg={c.dim}>
            Only hooks from managed settings can run. User-defined hooks from
            ~/.claude/settings.json, .claude/settings.json, and
            .claude/settings.local.json are blocked.
          </text>
        </box>
      )}
      <box marginBottom={1}>
        <text fg={c.dim}>
          \u2139 This menu is read-only. To add or modify hooks, edit
          settings.json directly or ask Claude. <span fg={c.info}>{DOCS_URL}</span>
        </text>
      </box>
      <box flexDirection="column">
        {events.map((name, i) => {
          const isSelected = i === cursor
          const count = hooksByEvent[name] ?? 0
          const metadata = hookEventMetadata[name]
          return (
            <box key={name} flexDirection="column">
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` ${name} `}</strong>
                {count > 0 && <span fg={isSelected ? c.bg : c.info}>{` (${count})`}</span>}
              </text>
              <text fg={c.dim}>{`   ${metadata.summary}`}</text>
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>Up/Down to move \u00B7 Enter to open \u00B7 Esc to close</text>
      </box>
    </box>
  )
}

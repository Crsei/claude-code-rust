import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import type {
  HookEvent,
  HookEventMetadata,
  IndividualHookConfig,
} from './types.js'
import { getHookDisplayText, hookSourceHeaderDisplayString } from './types.js'

/**
 * Final read-only list of hooks for a chosen event+matcher.
 *
 * OpenTUI-native port of the upstream `hooks/SelectHookMode`
 * (`ui/examples/upstream-patterns/src/components/hooks/SelectHookMode.tsx`).
 * Selecting a hook shows its details via `ViewHookMode`.
 */

type Props = {
  selectedEvent: HookEvent
  selectedMatcher: string | null
  hooksForSelectedMatcher: IndividualHookConfig[]
  hookEventMetadata: HookEventMetadata
  onSelect: (hook: IndividualHookConfig) => void
  onCancel: () => void
}

export function SelectHookMode({
  selectedEvent,
  selectedMatcher,
  hooksForSelectedMatcher,
  hookEventMetadata,
  onSelect,
  onCancel,
}: Props) {
  const [cursor, setCursor] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = event.sequence ?? name ?? ''
    if (name === 'escape') {
      onCancel()
      return
    }
    if (hooksForSelectedMatcher.length === 0) return
    if (name === 'up' || input === 'k') {
      setCursor(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setCursor(prev => Math.min(hooksForSelectedMatcher.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const selected = hooksForSelectedMatcher[cursor]
      if (selected) onSelect(selected)
    }
  })

  const title =
    hookEventMetadata.matcherMetadata !== undefined
      ? `${selectedEvent} \u00B7 Matcher: ${selectedMatcher || '(all)'}`
      : selectedEvent

  if (hooksForSelectedMatcher.length === 0) {
    return (
      <box
        position="absolute"
        top={2}
        left={2}
        right={2}
        bottom={2}
        flexDirection="column"
        borderStyle="rounded"
        borderColor={c.warning}
        title={title}
        titleAlignment="left"
        paddingX={2}
        paddingY={1}
      >
        <text fg={c.dim}>{hookEventMetadata.description}</text>
        <box marginTop={1} flexDirection="column">
          <text fg={c.dim}>No hooks configured for this event.</text>
          <text fg={c.dim}>
            To add hooks, edit settings.json directly or ask Claude.
          </text>
        </box>
        <box marginTop={1}>
          <text fg={c.dim}>Esc to go back</text>
        </box>
      </box>
    )
  }

  return (
    <box
      position="absolute"
      top={2}
      left={2}
      right={2}
      bottom={2}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title={title}
      titleAlignment="left"
      paddingX={2}
      paddingY={1}
    >
      <text fg={c.dim}>{hookEventMetadata.description}</text>
      <box marginTop={1} flexDirection="column">
        {hooksForSelectedMatcher.map((hook, i) => {
          const isSelected = i === cursor
          const typeLabel = hook.config.type
          const displayText = getHookDisplayText(hook.config)
          const sourceLabel =
            hook.source === 'pluginHook' && hook.pluginName
              ? `${hookSourceHeaderDisplayString(hook.source)} (${hook.pluginName})`
              : hookSourceHeaderDisplayString(hook.source)
          return (
            <box key={`hook-${i}`} flexDirection="column">
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` [${typeLabel}] ${displayText} `}</strong>
              </text>
              <text fg={c.dim}>{`   ${sourceLabel}`}</text>
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>Up/Down to move \u00B7 Enter for details \u00B7 Esc to go back</text>
      </box>
    </box>
  )
}

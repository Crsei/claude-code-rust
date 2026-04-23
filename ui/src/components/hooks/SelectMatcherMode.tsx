import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import type { HookEvent, HookSource, IndividualHookConfig } from './types.js'
import { hookSourceInlineDisplayString, plural } from './types.js'

/**
 * Matcher picker for a selected hook event.
 *
 * OpenTUI-native port of the upstream `hooks/SelectMatcherMode`
 * (`ui/examples/upstream-patterns/src/components/hooks/SelectMatcherMode.tsx`).
 * Read-only: selecting a matcher drills into the list of individual
 * hooks configured for that event+matcher pair.
 */

type MatcherWithSource = {
  matcher: string
  sources: HookSource[]
  hookCount: number
}

type Props = {
  selectedEvent: HookEvent
  matchersForSelectedEvent: string[]
  hooksByEventAndMatcher: Record<HookEvent, Record<string, IndividualHookConfig[]>>
  eventDescription: string
  onSelect: (matcher: string) => void
  onCancel: () => void
}

export function SelectMatcherMode({
  selectedEvent,
  matchersForSelectedEvent,
  hooksByEventAndMatcher,
  eventDescription,
  onSelect,
  onCancel,
}: Props) {
  const matchersWithSources: MatcherWithSource[] = useMemo(() => {
    return matchersForSelectedEvent.map(matcher => {
      const hooks = hooksByEventAndMatcher[selectedEvent]?.[matcher] ?? []
      const sources = Array.from(new Set(hooks.map(h => h.source)))
      return {
        matcher,
        sources,
        hookCount: hooks.length,
      }
    })
  }, [matchersForSelectedEvent, hooksByEventAndMatcher, selectedEvent])

  const [cursor, setCursor] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = event.sequence ?? name ?? ''
    if (name === 'escape') {
      onCancel()
      return
    }
    if (matchersWithSources.length === 0) return
    if (name === 'up' || input === 'k') {
      setCursor(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setCursor(prev => Math.min(matchersWithSources.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const selected = matchersWithSources[cursor]
      if (selected) onSelect(selected.matcher)
    }
  })

  const title = `${selectedEvent} \u00B7 Matchers`

  if (matchersForSelectedEvent.length === 0) {
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
        <text fg={c.dim}>{eventDescription}</text>
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
      <text fg={c.dim}>{eventDescription}</text>
      <box marginTop={1} flexDirection="column">
        {matchersWithSources.map((item, i) => {
          const isSelected = i === cursor
          const sourceText = item.sources
            .map(hookSourceInlineDisplayString)
            .join(', ')
          const matcherLabel = item.matcher || '(all)'
          return (
            <box key={`${item.matcher}-${i}`} flexDirection="column">
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` [${sourceText}] ${matcherLabel} `}</strong>
              </text>
              <text fg={c.dim}>
                {`   ${item.hookCount} ${plural(item.hookCount, 'hook')}`}
              </text>
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>Up/Down to move \u00B7 Enter to open \u00B7 Esc to go back</text>
      </box>
    </box>
  )
}

import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import type {
  HookEvent,
  HookEventMetadata,
  IndividualHookConfig,
} from './types.js'
import { plural } from './types.js'
import { SelectEventMode } from './SelectEventMode.js'
import { SelectHookMode } from './SelectHookMode.js'
import { SelectMatcherMode } from './SelectMatcherMode.js'
import { ViewHookMode } from './ViewHookMode.js'

/**
 * Orchestrator for the `/hooks` read-only configuration browser.
 *
 * OpenTUI-native port of the upstream `hooks/HooksConfigMenu`
 * (`ui/examples/upstream-patterns/src/components/hooks/HooksConfigMenu.tsx`).
 * Upstream pulled live state from `AppState`, settings readers, the
 * `hooksConfigManager` grouping helpers, and `useSettingsChange`. The
 * Lite port takes a flat `HooksSnapshot` from the caller \u2014 the Rust
 * backend builds the snapshot from policy / user / project / local /
 * plugin settings and ships it through IPC.
 *
 * The machine has four modes (select-event \u2192 select-matcher \u2192
 * select-hook \u2192 view-hook) with Esc routing back up. When all hooks
 * are disabled via `settings.disableAllHooks`, a single informational
 * frame is rendered instead.
 */

export type HooksSnapshot = {
  /** Fully-expanded hook list \u2014 one entry per configured hook. */
  hooks: IndividualHookConfig[]
  /** Metadata per event, includes `matcherMetadata` for tool-matcher events. */
  eventMetadata: Record<HookEvent, HookEventMetadata>
  /** Combined tool name list (builtin + MCP). Kept for
   *  `getMatcherMetadata`-style lookups. */
  toolNames: string[]
  /** True when `settings.disableAllHooks === true`. */
  hooksDisabled: boolean
  /** True when a managed policy set `disableAllHooks`. */
  disabledByPolicy: boolean
  /** True when a managed policy enabled `allowManagedHooksOnly`. */
  restrictedByPolicy: boolean
}

type ModeState =
  | { mode: 'select-event' }
  | { mode: 'select-matcher'; event: HookEvent }
  | { mode: 'select-hook'; event: HookEvent; matcher: string }
  | { mode: 'view-hook'; event: HookEvent; hook: IndividualHookConfig }

type Props = {
  snapshot: HooksSnapshot
  onExit: () => void
}

function groupHooks(
  hooks: IndividualHookConfig[],
): Record<HookEvent, Record<string, IndividualHookConfig[]>> {
  const grouped: Partial<Record<HookEvent, Record<string, IndividualHookConfig[]>>> = {}
  for (const hook of hooks) {
    const byEvent = grouped[hook.event] ?? {}
    const matcherKey = hook.matcher ?? ''
    const list = byEvent[matcherKey] ?? []
    list.push(hook)
    byEvent[matcherKey] = list
    grouped[hook.event] = byEvent
  }
  return grouped as Record<HookEvent, Record<string, IndividualHookConfig[]>>
}

function sortedMatchersFor(
  grouped: Record<HookEvent, Record<string, IndividualHookConfig[]>>,
  event: HookEvent,
): string[] {
  const byMatcher = grouped[event] ?? {}
  return Object.keys(byMatcher).sort((a, b) => {
    if (a === '' && b !== '') return -1
    if (a !== '' && b === '') return 1
    return a.localeCompare(b)
  })
}

export function HooksConfigMenu({ snapshot, onExit }: Props) {
  const [modeState, setModeState] = useState<ModeState>({ mode: 'select-event' })

  const grouped = useMemo(() => groupHooks(snapshot.hooks), [snapshot.hooks])

  const { hooksByEvent, totalHooksCount } = useMemo(() => {
    const byEvent: Partial<Record<HookEvent, number>> = {}
    let total = 0
    for (const [event, matchers] of Object.entries(grouped) as Array<
      [HookEvent, Record<string, IndividualHookConfig[]>]
    >) {
      const count = Object.values(matchers).reduce((sum, list) => sum + list.length, 0)
      byEvent[event] = count
      total += count
    }
    return { hooksByEvent: byEvent, totalHooksCount: total }
  }, [grouped])

  const selectedEvent = 'event' in modeState ? modeState.event : 'PreToolUse'
  const selectedMatcher = 'matcher' in modeState ? modeState.matcher : null

  const sortedMatchers = useMemo(
    () => sortedMatchersFor(grouped, selectedEvent),
    [grouped, selectedEvent],
  )

  const hooksForSelectedMatcher = useMemo(() => {
    if (selectedMatcher === null) return []
    return grouped[selectedEvent]?.[selectedMatcher] ?? []
  }, [grouped, selectedEvent, selectedMatcher])

  // The disabled-by-policy screen has its own Esc handling; wire the rest
  // here so the mode machine stays in one place.
  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (!snapshot.hooksDisabled) return
    if (event.name === 'escape' || event.name === 'enter' || event.name === 'return') {
      onExit()
    }
  })

  if (snapshot.hooksDisabled) {
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
        title="Hook Configuration \u00B7 Disabled"
        titleAlignment="left"
        paddingX={2}
        paddingY={1}
      >
        <text>
          All hooks are currently <strong>disabled</strong>
          {snapshot.disabledByPolicy && ' by a managed settings file'}. You have{' '}
          <strong>{totalHooksCount}</strong> configured {plural(totalHooksCount, 'hook')} that{' '}
          {plural(totalHooksCount, 'is', 'are')} not running.
        </text>
        <box marginTop={1} flexDirection="column">
          <text fg={c.dim}>When hooks are disabled:</text>
          <text fg={c.dim}>\u00B7 No hook commands will execute</text>
          <text fg={c.dim}>\u00B7 StatusLine will not be displayed</text>
          <text fg={c.dim}>\u00B7 Tool operations will proceed without hook validation</text>
        </box>
        {!snapshot.disabledByPolicy && (
          <box marginTop={1}>
            <text fg={c.dim}>
              To re-enable hooks, remove &quot;disableAllHooks&quot; from settings.json or ask
              Claude.
            </text>
          </box>
        )}
        <box marginTop={1}>
          <text fg={c.dim}>Esc to close</text>
        </box>
      </box>
    )
  }

  switch (modeState.mode) {
    case 'select-event':
      return (
        <SelectEventMode
          hookEventMetadata={snapshot.eventMetadata}
          hooksByEvent={hooksByEvent}
          totalHooksCount={totalHooksCount}
          restrictedByPolicy={snapshot.restrictedByPolicy}
          onSelectEvent={event => {
            const hasMatchers =
              snapshot.eventMetadata[event]?.matcherMetadata !== undefined
            if (hasMatchers) {
              setModeState({ mode: 'select-matcher', event })
            } else {
              setModeState({ mode: 'select-hook', event, matcher: '' })
            }
          }}
          onCancel={onExit}
        />
      )
    case 'select-matcher':
      return (
        <SelectMatcherMode
          selectedEvent={modeState.event}
          matchersForSelectedEvent={sortedMatchers}
          hooksByEventAndMatcher={grouped}
          eventDescription={snapshot.eventMetadata[modeState.event]?.description ?? ''}
          onSelect={matcher => {
            setModeState({
              mode: 'select-hook',
              event: modeState.event,
              matcher,
            })
          }}
          onCancel={() => setModeState({ mode: 'select-event' })}
        />
      )
    case 'select-hook':
      return (
        <SelectHookMode
          selectedEvent={modeState.event}
          selectedMatcher={modeState.matcher}
          hooksForSelectedMatcher={hooksForSelectedMatcher}
          hookEventMetadata={snapshot.eventMetadata[modeState.event]!}
          onSelect={hook => {
            setModeState({ mode: 'view-hook', event: modeState.event, hook })
          }}
          onCancel={() => {
            const hasMatchers =
              snapshot.eventMetadata[modeState.event]?.matcherMetadata !== undefined
            if (hasMatchers) {
              setModeState({ mode: 'select-matcher', event: modeState.event })
            } else {
              setModeState({ mode: 'select-event' })
            }
          }}
        />
      )
    case 'view-hook': {
      const { event, hook } = modeState
      const eventSupportsMatcher =
        snapshot.eventMetadata[event]?.matcherMetadata !== undefined
      return (
        <ViewHookMode
          selectedHook={hook}
          eventSupportsMatcher={eventSupportsMatcher}
          onCancel={() => {
            setModeState({
              mode: 'select-hook',
              event,
              matcher: hook.matcher || '',
            })
          }}
        />
      )
    }
  }
}

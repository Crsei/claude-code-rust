import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import type { EffortLevel } from './EffortIndicator.js'
import { effortLevelToSymbol, normalizeEffortLevel } from './EffortIndicator.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/EffortCallout.tsx`.
 *
 * Upstream uses this dialog to announce the new default effort level
 * for Opus 4.6 and let subscribers pick medium / high / low. The logic
 * depends on:
 *  - `getOpusDefaultEffortConfig()` — remote-config-driven copy and
 *    enabled flag.
 *  - `getDefaultEffortForModel(model)` — tier + model lookup.
 *  - `getGlobalConfig()` / `saveGlobalConfig()` — "already seen" state.
 *  - `updateSettingsForSource()` — persistence for the new choice.
 *
 * None of those helpers live in the cc-rust frontend, so this port
 * accepts the key inputs as props (title, description, defaultLevel)
 * and emits the user's choice through `onDone`. `shouldShowEffortCallout`
 * is similarly a pure predicate the caller feeds with the relevant
 * subscriber + model state so the decision stays outside the component.
 */

export type EffortCalloutSelection = EffortLevel | 'dismiss'

type Props = {
  /** Dialog header — mirrors upstream `dialogTitle`. */
  title: string
  /** One or two-line body copy — mirrors upstream `dialogDescription`. */
  description: string
  /** Level to preselect — defaults to `medium`. */
  defaultLevel?: EffortLevel
  /** Called with the picked level or `'dismiss'`. */
  onDone: (selection: EffortCalloutSelection) => void
  /** Auto-dismiss timeout in ms. Defaults to 30s, matching upstream. */
  autoDismissMs?: number
}

type Option = {
  value: EffortLevel
  label: string
  hotkey: string
}

const DEFAULT_AUTO_DISMISS_MS = 30_000

const OPTIONS: Option[] = [
  { value: 'medium', label: 'Medium (recommended)', hotkey: 'm' },
  { value: 'high', label: 'High', hotkey: 'h' },
  { value: 'low', label: 'Low', hotkey: 'l' },
]

export function EffortCallout({
  title,
  description,
  defaultLevel = 'medium',
  onDone,
  autoDismissMs = DEFAULT_AUTO_DISMISS_MS,
}: Props) {
  const initialIndex = OPTIONS.findIndex(opt => opt.value === defaultLevel)
  const [selected, setSelected] = useState(Math.max(0, initialIndex))
  const safeIndex = Math.max(0, Math.min(selected, OPTIONS.length - 1))

  const onDoneRef = useRef(onDone)
  useEffect(() => {
    onDoneRef.current = onDone
  })

  const cancel = useCallback(() => onDoneRef.current('dismiss'), [])

  useEffect(() => {
    if (!Number.isFinite(autoDismissMs) || autoDismissMs <= 0) return undefined
    const timer = setTimeout(cancel, autoDismissMs)
    return () => clearTimeout(timer)
  }, [cancel, autoDismissMs])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input) {
      const match = OPTIONS.findIndex(opt => opt.hotkey === input)
      if (match >= 0) {
        onDoneRef.current(OPTIONS[match]!.value)
        return
      }
    }

    if (name === 'escape') {
      cancel()
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setSelected(Math.min(OPTIONS.length - 1, safeIndex + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      onDoneRef.current(OPTIONS[safeIndex]!.value)
    }
  })

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      title={title}
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text selectable>{description}</text>
      <box marginTop={1}>
        <text fg={c.dim}>
          <span fg={c.accent}>{effortLevelToSymbol('low')}</span> low ·{' '}
          <span fg={c.accent}>{effortLevelToSymbol('medium')}</span> medium ·{' '}
          <span fg={c.accent}>{effortLevelToSymbol('high')}</span> high
        </text>
      </box>
      <box marginTop={1} flexDirection="column">
        {OPTIONS.map((opt, i) => {
          const isSelected = i === safeIndex
          return (
            <box key={opt.value} flexDirection="row">
              <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                <strong>{` ${effortLevelToSymbol(opt.value)} ${opt.label} `}</strong>
              </text>
              <text fg={c.dim}> ({opt.hotkey})</text>
            </box>
          )
        })}
      </box>
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Up/Down to move · Enter to confirm · Esc to dismiss (auto-dismiss in 30s)
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}

/**
 * Pure predicate version of upstream's `shouldShowEffortCallout`.
 * Instead of poking `getGlobalConfig` and the subscriber helpers, the
 * caller hands over an already-resolved audience + state record and the
 * function decides whether the dialog should render.
 *
 * Keeping the decision separate lets cc-rust evolve its subscriber
 * detection (currently driven by the backend auth payload) without
 * touching this component.
 */
export function shouldShowEffortCallout(ctx: {
  /** The active model string — "opus-4-6", etc. */
  model: string
  /** `true` if the callout was already shown. */
  v2Dismissed: boolean
  /** Number of session starts seen so far. */
  numStartups: number
  /** Whether the caller's subscriber check says the user is Pro. */
  isPro: boolean
  /** Whether the caller's subscriber check says the user is Max/Team. */
  isMaxOrTeam: boolean
  /** Whether an old (v1) dismissal exists. */
  v1Dismissed: boolean
  /** Whether remote config has flipped the callout feature on. */
  enabledRemote: boolean
}): boolean {
  if (!ctx.model.toLowerCase().includes('opus-4-6')) return false
  if (ctx.v2Dismissed) return false
  if (ctx.numStartups <= 1) return false
  if (ctx.isPro) {
    if (ctx.v1Dismissed) return false
    return ctx.enabledRemote
  }
  if (ctx.isMaxOrTeam) return ctx.enabledRemote
  return false
}

/** Convenience: convert a free-form effort value to `EffortLevel | undefined`. */
export { normalizeEffortLevel }

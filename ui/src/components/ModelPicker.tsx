import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/ModelPicker.tsx`.
 *
 * Upstream couples the picker to the full model catalog, effort
 * cycling, Fast Mode, and `[1m]` context toggling — all of which live
 * behind the Ink app state. The Lite port keeps the visible chrome
 * (header, option list, effort row, 1M row, Fast Mode notice) and the
 * selection flow; consumers pass the option list + effort state and
 * receive the resulting `(model, effort)` pair on submit.
 */

export type EffortLevel = 'low' | 'medium' | 'high' | 'max'

export type ModelPickerOption = {
  value: string
  label: string
  description?: string
  /** When true, `[1m]` 1M-context toggle is offered for this model. */
  supports1M?: boolean
  /** When true, the effort row is enabled for this model. */
  supportsEffort?: boolean
  /** When true, the `max` effort level is exposed. */
  supportsMaxEffort?: boolean
  defaultEffort?: EffortLevel
}

const NO_PREFERENCE = '__NO_PREFERENCE__'

type Props = {
  initial: string | null
  sessionModel?: string
  options: ModelPickerOption[]
  currentEffort?: EffortLevel
  /** Overrides the dim header line below "Select model". */
  headerText?: string
  /** Renders the "Fast mode" banner when the caller decides it's
   *  appropriate (feature gated in upstream). */
  showFastModeNotice?: boolean
  fastModeModelDisplay?: string
  fastModeOn?: boolean
  /** Standalone renders the picker inside a bordered dialog pane. */
  isStandaloneCommand?: boolean
  onSelect: (model: string | null, effort: EffortLevel | undefined) => void
  onCancel?: () => void
}

function effortSymbol(effort: EffortLevel | undefined): string {
  switch (effort) {
    case 'max':
      return '\u25C6\u25C6\u25C6\u25C6'
    case 'high':
      return '\u25C6\u25C6\u25C6'
    case 'medium':
      return '\u25C6\u25C6'
    case 'low':
      return '\u25C6'
    default:
      return '\u25C7'
  }
}

function cycleEffort(
  current: EffortLevel,
  direction: 'left' | 'right',
  includeMax: boolean,
): EffortLevel {
  const levels: EffortLevel[] = includeMax
    ? ['low', 'medium', 'high', 'max']
    : ['low', 'medium', 'high']
  const idx = levels.indexOf(current)
  const resolved = idx >= 0 ? idx : levels.indexOf('high')
  if (direction === 'right') {
    return levels[(resolved + 1) % levels.length]!
  }
  return levels[(resolved - 1 + levels.length) % levels.length]!
}

function capitalize(s: string): string {
  return s.length === 0 ? s : s[0]!.toUpperCase() + s.slice(1)
}

export function ModelPicker({
  initial,
  sessionModel,
  options,
  currentEffort,
  headerText,
  showFastModeNotice = false,
  fastModeModelDisplay,
  fastModeOn = false,
  isStandaloneCommand = false,
  onSelect,
  onCancel,
}: Props) {
  const allOptions = useMemo<ModelPickerOption[]>(() => {
    const withNoPref = options.slice()
    if (!withNoPref.some(o => o.value === NO_PREFERENCE)) {
      withNoPref.unshift({
        value: NO_PREFERENCE,
        label: 'No preference',
        description: 'Use the default model for this session.',
      })
    }
    return withNoPref
  }, [options])

  const initialValue = initial === null ? NO_PREFERENCE : initial
  const initialIdx = Math.max(
    0,
    allOptions.findIndex(o => o.value === initialValue),
  )
  const [selected, setSelected] = useState(initialIdx)
  const [effort, setEffort] = useState<EffortLevel | undefined>(currentEffort)
  const [toggled1M, setToggled1M] = useState<Set<string>>(() => {
    const set = new Set<string>()
    if (initial && /\[1m\]/i.test(initial)) {
      set.add(initial.replace(/\[1m\]/i, ''))
    }
    return set
  })
  const [hasToggledEffort, setHasToggledEffort] = useState(false)

  useEffect(() => {
    const next = Math.max(
      0,
      allOptions.findIndex(o => o.value === initialValue),
    )
    setSelected(next)
  }, [allOptions, initialValue])

  const focused = allOptions[selected]

  const focusedSupportsEffort = !!focused?.supportsEffort
  const focusedSupportsMax = !!focused?.supportsMaxEffort
  const focusedDefault: EffortLevel = focused?.defaultEffort ?? 'high'
  const displayEffort =
    effort === 'max' && !focusedSupportsMax ? 'high' : effort ?? focusedDefault
  const is1MMarked =
    focused != null &&
    focused.value !== NO_PREFERENCE &&
    toggled1M.has(focused.value)

  const handleCycle = useCallback(
    (direction: 'left' | 'right') => {
      if (!focusedSupportsEffort) return
      setEffort(prev =>
        cycleEffort(prev ?? focusedDefault, direction, focusedSupportsMax),
      )
      setHasToggledEffort(true)
    },
    [focusedSupportsEffort, focusedSupportsMax, focusedDefault],
  )

  const handleToggle1M = useCallback(() => {
    if (!focused || focused.value === NO_PREFERENCE || !focused.supports1M) return
    setToggled1M(prev => {
      const next = new Set(prev)
      if (next.has(focused.value)) next.delete(focused.value)
      else next.add(focused.value)
      return next
    })
  }, [focused])

  const commit = useCallback(
    (idx: number) => {
      const opt = allOptions[idx]
      if (!opt) return
      const effortToSend =
        hasToggledEffort && opt.supportsEffort ? effort : undefined
      if (opt.value === NO_PREFERENCE) {
        onSelect(null, effortToSend)
        return
      }
      const wants1M = toggled1M.has(opt.value)
      const base = opt.value.replace(/\[1m\]/i, '')
      onSelect(wants1M ? `${base}[1m]` : base, effortToSend)
    },
    [allOptions, effort, hasToggledEffort, toggled1M, onSelect],
  )

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
      commit(selected)
      return
    }
    if (name === 'up' || key === 'k') {
      setSelected(idx => Math.max(0, idx - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setSelected(idx => Math.min(allOptions.length - 1, idx + 1))
      return
    }
    if (name === 'left') {
      handleCycle('left')
      return
    }
    if (name === 'right') {
      handleCycle('right')
      return
    }
    if (key === ' ') {
      handleToggle1M()
    }
  })

  const visibleCount = Math.min(10, allOptions.length)
  const hiddenCount = Math.max(0, allOptions.length - visibleCount)
  const slice = allOptions.slice(0, visibleCount)

  const body = (
    <box flexDirection="column">
      <box flexDirection="column" marginBottom={1}>
        <strong>
          <text fg={c.accent}>Select model</text>
        </strong>
        <text fg={c.dim}>
          {headerText ??
            'Switch between Claude models. Applies to this session and future Claude Code sessions.'}
        </text>
        {sessionModel && (
          <text fg={c.dim}>
            Currently using {sessionModel} for this session. Selecting a model
            will undo this.
          </text>
        )}
      </box>

      <box flexDirection="column" marginBottom={1}>
        {slice.map((opt, i) => {
          const isSelected = i === selected
          return (
            <box key={opt.value} flexDirection="column">
              <box flexDirection="row">
                <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                  <strong>{` ${opt.label} `}</strong>
                </text>
              </box>
              {opt.description && isSelected && (
                <box paddingLeft={3}>
                  <text fg={c.dim}>{opt.description}</text>
                </box>
              )}
            </box>
          )
        })}
        {hiddenCount > 0 && (
          <box paddingLeft={3}>
            <text fg={c.dim}>and {hiddenCount} more…</text>
          </box>
        )}
      </box>

      <box flexDirection="column" marginBottom={1}>
        {focusedSupportsEffort ? (
          <text fg={c.dim}>
            <span fg={c.accent}>{effortSymbol(displayEffort)}</span>{' '}
            {capitalize(displayEffort)} effort
            {displayEffort === focusedDefault ? ' (default)' : ''}{' '}
            <span fg={c.muted}>\u2190 \u2192 to adjust</span>
          </text>
        ) : (
          <text fg={c.muted}>
            {effortSymbol(undefined)} Effort not supported
            {focused?.label ? ` for ${focused.label}` : ''}
          </text>
        )}
        {focused?.supports1M && is1MMarked ? (
          <text fg={c.dim}>
            <span fg={c.accent}>{effortSymbol('high')}</span> 1M context on
            <span fg={c.muted}> · Space to toggle</span>
          </text>
        ) : (
          <text fg={c.muted}>
            {effortSymbol(undefined)} 1M context off
            {focused?.label ? ` for ${focused.label}` : ''}
          </text>
        )}
      </box>

      {showFastModeNotice && fastModeModelDisplay && (
        <box marginBottom={1}>
          {fastModeOn ? (
            <text fg={c.dim}>
              Fast mode is <strong>ON</strong> and available with{' '}
              {fastModeModelDisplay} only (/fast). Switching to other models
              turns off fast mode.
            </text>
          ) : (
            <text fg={c.dim}>
              Use <strong>/fast</strong> to turn on Fast mode (
              {fastModeModelDisplay} only).
            </text>
          )}
        </box>
      )}

      <text fg={c.dim}>Enter to confirm · Esc to cancel</text>
    </box>
  )

  if (!isStandaloneCommand) return body

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
    >
      {body}
    </box>
  )
}

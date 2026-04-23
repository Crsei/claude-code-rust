import React from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'
import type { OptionWithDescription } from './select.js'
import {
  useMultiSelectState,
  type UseMultiSelectStateProps,
} from './use-multi-select-state.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/SelectMulti.tsx`.
 *
 * Multi-select list box: rows render a `[x] label` / `[ ] label` gutter,
 * Space toggles, Enter submits. Upstream supports richer layouts and
 * commit-mode hotkeys — the Lite port covers the core contract:
 *
 *  - `options` + `defaultValues` seed the selection set
 *  - ↑/↓ or j/k navigates
 *  - Space / Tab toggles the focused option
 *  - Enter calls `onSubmit` with the current selection (gated by
 *    `minSelected`)
 *  - Esc calls `onCancel`
 */

type Props<T> = UseMultiSelectStateProps<T> & {
  isDisabled?: boolean
  /** Layout hint matching upstream — `compact-vertical` shows the
   *  description below the label, otherwise it trails inline. */
  layout?: 'compact' | 'compact-vertical' | 'expanded'
  /** When true, hides the `[x] / [ ]` gutter. Upstream consumers use
   *  this for custom row rendering. */
  hideCheckbox?: boolean
}

function renderCheckbox(checked: boolean): string {
  return checked ? '[\u2713]' : '[ ]'
}

export function SelectMulti<T>({
  options,
  defaultValues,
  visibleOptionCount = 10,
  onChange,
  onSubmit,
  onCancel,
  onFocus,
  focusValue,
  minSelected = 0,
  maxSelected,
  isDisabled = false,
  layout = 'compact',
  hideCheckbox = false,
}: Props<T>): React.ReactNode {
  const state = useMultiSelectState<T>({
    options,
    defaultValues,
    visibleOptionCount,
    onChange,
    onSubmit,
    onCancel,
    onFocus,
    focusValue,
    minSelected,
    maxSelected,
  })

  useKeyboard((event: KeyEvent) => {
    if (isDisabled || event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      state.onCancel?.()
      return
    }
    if (name === 'return' || name === 'enter') {
      state.submit()
      return
    }
    if (name === 'up' || key === 'k') {
      state.focusPreviousOption()
      return
    }
    if (name === 'down' || key === 'j') {
      state.focusNextOption()
      return
    }
    if (name === 'tab' || key === ' ') {
      state.toggleFocused()
      return
    }
    if (name === 'pageup') state.focusPreviousPage()
    else if (name === 'pagedown') state.focusNextPage()
  })

  return (
    <box flexDirection="column">
      {state.visibleOptions.map(opt => {
        const isFocused = opt.index === state.focusedIndex - 1
        const checked = state.isSelected(opt.value as T)
        const description =
          layout === 'compact-vertical' ? opt.description : undefined
        const inlineDesc =
          layout !== 'compact-vertical' ? opt.description : undefined
        return (
          <box key={String(opt.value)} flexDirection="column">
            <box flexDirection="row" gap={1}>
              <text fg={isFocused ? c.accent : c.dim}>
                {isFocused ? '\u276F' : ' '}
              </text>
              {!hideCheckbox && (
                <text fg={checked ? c.success : c.dim}>
                  {renderCheckbox(checked)}
                </text>
              )}
              {isFocused ? (
                <strong>
                  <text fg={c.textBright}>{opt.label}</text>
                </strong>
              ) : (
                <text>{opt.label}</text>
              )}
              {inlineDesc && <text fg={c.dim}>{inlineDesc}</text>}
            </box>
            {description && (
              <box paddingLeft={4}>
                <text fg={c.dim}>{description}</text>
              </box>
            )}
          </box>
        )
      })}
      <box marginTop={1}>
        <text fg={c.dim}>
          Space toggle · Enter confirm · Esc cancel
          {minSelected > 0 && ` · min ${minSelected}`}
          {maxSelected !== undefined && ` · max ${maxSelected}`}
        </text>
      </box>
    </box>
  )
}

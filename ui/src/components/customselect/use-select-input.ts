import { useCallback } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import type { SelectState } from './use-select-state.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/use-select-input.ts`.
 *
 * Upstream's version cooperates with Ink's `useInput` + text-mode
 * input-buffering; the Lite port talks to `@opentui/react`'s
 * `useKeyboard`. Keystroke contract preserved:
 *
 *  - ↑/↓ or j/k         → previous/next focus
 *  - PageUp/PageDown    → previous/next page
 *  - Home / g           → jump to first (first enabled option)
 *  - End / G            → jump to last
 *  - Enter              → commit focused via `onChange`
 *  - Esc                → `onCancel`
 *  - Hotkey characters  → caller-controlled (upstream exposes
 *    `isCancelable` and `disableNavigation`; we keep those flags).
 */

export type UseSelectInputProps<T> = {
  isDisabled?: boolean
  state: SelectState<T>
  /** When true, Enter does not fire `onChange`. Upstream uses this for
   *  pickers that render a Select but commit elsewhere (e.g. a wizard
   *  advance button). */
  disableSelection?: boolean
  /** When false, the hook swallows Esc instead of firing `onCancel`.
   *  Upstream wizards rely on this for non-cancelable steps. */
  isCancelable?: boolean
}

export function useSelectInput<T>({
  isDisabled = false,
  state,
  disableSelection = false,
  isCancelable = true,
}: UseSelectInputProps<T>): void {
  const commit = useCallback(() => {
    if (disableSelection) return
    state.selectFocusedOption()
    if (state.focusedValue !== undefined) {
      state.onChange?.(state.focusedValue)
    }
  }, [state, disableSelection])

  useKeyboard((event: KeyEvent) => {
    if (isDisabled || event.eventType === 'release') return
    if (state.isInInput) return

    const name = event.name ?? ''
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      if (isCancelable) state.onCancel?.()
      return
    }
    if (name === 'return' || name === 'enter') {
      commit()
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
    if (name === 'pageup') {
      state.focusPreviousPage()
      return
    }
    if (name === 'pagedown') {
      state.focusNextPage()
      return
    }
    if (name === 'home' || key === 'g') {
      const first = state.options[0]?.value as T | undefined
      state.focusOption(first)
      return
    }
    if (name === 'end' || (seq === 'G' && event.shift)) {
      const last = state.options[state.options.length - 1]?.value as T | undefined
      state.focusOption(last)
      return
    }
  })
}

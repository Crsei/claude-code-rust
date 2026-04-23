import { useCallback, useState } from 'react'
import type { OptionWithDescription } from './select.js'
import { useSelectNavigation, type NavigationState } from './use-select-navigation.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/use-multi-select-state.ts`.
 *
 * Multi-select state machine — same navigation API as `useSelectState`
 * plus a `selectedValues` set, `toggle()` helper, and `submit()` that
 * forwards the chosen values to `onSubmit`. Upstream layers rich
 * behaviours like required-minimums and commit hotkeys; the Lite port
 * exposes the knobs (`minSelected`, `maxSelected`, `isCommitted`) so
 * the calling dialog can enforce its own rules.
 */

export type UseMultiSelectStateProps<T> = {
  visibleOptionCount?: number
  options: OptionWithDescription<T>[]
  defaultValues?: T[]
  onChange?: (values: T[]) => void
  onSubmit?: (values: T[]) => void
  onCancel?: () => void
  onFocus?: (value: T) => void
  focusValue?: T
  minSelected?: number
  maxSelected?: number
}

export type MultiSelectState<T> = NavigationState<T> & {
  selectedValues: Set<T>
  isSelected: (value: T) => boolean
  toggle: (value: T) => void
  toggleFocused: () => void
  submit: () => void
  onCancel?: () => void
}

export function useMultiSelectState<T>({
  visibleOptionCount = 5,
  options,
  defaultValues = [],
  onChange,
  onSubmit,
  onCancel,
  onFocus,
  focusValue,
  minSelected = 0,
  maxSelected,
}: UseMultiSelectStateProps<T>): MultiSelectState<T> {
  const [selected, setSelected] = useState<Set<T>>(() => new Set(defaultValues))

  const navigation = useSelectNavigation<T>({
    visibleOptionCount,
    options,
    initialFocusValue: defaultValues[0],
    onFocus,
    focusValue,
  })

  const toggle = useCallback(
    (value: T) => {
      setSelected(prev => {
        const next = new Set<T>(prev)
        if (next.has(value)) {
          next.delete(value)
        } else {
          if (maxSelected !== undefined && next.size >= maxSelected) {
            return prev
          }
          next.add(value)
        }
        onChange?.(Array.from(next) as T[])
        return next
      })
    },
    [onChange, maxSelected],
  )

  const toggleFocused = useCallback(() => {
    if (navigation.focusedValue !== undefined) toggle(navigation.focusedValue)
  }, [navigation.focusedValue, toggle])

  const submit = useCallback(() => {
    if (selected.size < minSelected) return
    onSubmit?.(Array.from(selected))
  }, [selected, onSubmit, minSelected])

  return {
    ...navigation,
    selectedValues: selected,
    isSelected: value => selected.has(value),
    toggle,
    toggleFocused,
    submit,
    onCancel,
  }
}

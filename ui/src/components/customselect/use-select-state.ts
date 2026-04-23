import { useCallback, useState } from 'react'
import type { OptionWithDescription } from './select.js'
import { useSelectNavigation } from './use-select-navigation.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/use-select-state.ts`.
 *
 * Composes `useSelectNavigation` with a local `value` slot that tracks
 * the last-selected option. Exposes the navigation API alongside the
 * `selectFocusedOption` commit helper — `Select` wires this up to the
 * Enter keystroke through `useSelectInput`.
 */

export type UseSelectStateProps<T> = {
  visibleOptionCount?: number
  options: OptionWithDescription<T>[]
  defaultValue?: T
  onChange?: (value: T) => void
  onCancel?: () => void
  onFocus?: (value: T) => void
  focusValue?: T
}

export type SelectState<T> = {
  focusedValue: T | undefined
  focusedIndex: number
  visibleFromIndex: number
  visibleToIndex: number
  value: T | undefined
  options: OptionWithDescription<T>[]
  visibleOptions: Array<OptionWithDescription<T> & { index: number }>
  isInInput: boolean
  focusNextOption: () => void
  focusPreviousOption: () => void
  focusNextPage: () => void
  focusPreviousPage: () => void
  focusOption: (value: T | undefined) => void
  selectFocusedOption: () => void
  onChange?: (value: T) => void
  onCancel?: () => void
}

export function useSelectState<T>({
  visibleOptionCount = 5,
  options,
  defaultValue,
  onChange,
  onCancel,
  onFocus,
  focusValue,
}: UseSelectStateProps<T>): SelectState<T> {
  const [value, setValue] = useState<T | undefined>(defaultValue)

  const navigation = useSelectNavigation<T>({
    visibleOptionCount,
    options,
    initialFocusValue: defaultValue,
    onFocus,
    focusValue,
  })

  const selectFocusedOption = useCallback(() => {
    if (navigation.focusedValue !== undefined) {
      setValue(navigation.focusedValue)
    }
  }, [navigation.focusedValue])

  return {
    ...navigation,
    value,
    selectFocusedOption,
    onChange,
    onCancel,
  }
}

import React, { type ReactNode, useEffect } from 'react'
import { c } from '../../theme.js'
import { SelectOption } from './select-option.js'
import { SelectInputOption } from './select-input-option.js'
import { useSelectInput } from './use-select-input.js'
import { useSelectState } from './use-select-state.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/select.tsx`.
 *
 * Upstream's 900+-line component carries a decade of Ink
 * special-casing (paste-mode scrolling, highlightText, layout variants,
 * input-mode re-focusing, etc). The Lite port keeps the public prop
 * surface so call sites don't need to change imports, and supports the
 * feature subset the Lite UI exercises today:
 *
 *  - rendered option list with focused / selected markers
 *  - `compact` (default), `expanded`, `compact-vertical` layouts
 *  - `inlineDescriptions` toggle
 *  - optional `type: 'input'` rows (see `SelectInputOption`)
 *  - keyboard navigation through `useSelectInput`
 *
 * Features not yet wired:
 *  - `highlightText` (substring highlight inside labels)
 *  - numeric prefix / `hideIndexes`
 *  - paste-mode scrolling hints
 *
 * These are additive and can be layered back in once Lite has a
 * caller that actually needs them.
 */

type BaseOption<T> = {
  description?: string
  dimDescription?: boolean
  label: ReactNode
  value: T
  disabled?: boolean
}

export type OptionWithDescription<T = string> =
  | (BaseOption<T> & { type?: 'text' })
  | (BaseOption<T> & {
      type: 'input'
      onChange: (value: string) => void
      placeholder?: string
      initialValue?: string
      allowEmptySubmitToCancel?: boolean
      showLabelWithValue?: boolean
      labelValueSeparator?: string
      resetCursorOnUpdate?: boolean
    })

export type SelectProps<T> = {
  readonly isDisabled?: boolean
  readonly disableSelection?: boolean
  readonly hideIndexes?: boolean
  readonly visibleOptionCount?: number
  readonly highlightText?: string
  readonly options: OptionWithDescription<T>[]
  readonly defaultValue?: T
  readonly onCancel?: () => void
  readonly onChange?: (value: T) => void
  readonly onFocus?: (value: T) => void
  readonly defaultFocusValue?: T
  readonly layout?: 'compact' | 'expanded' | 'compact-vertical'
  readonly inlineDescriptions?: boolean
  readonly focusValue?: T
  readonly isCancelable?: boolean
  /** Fires when an input-row toggles between display and edit states. */
  readonly onInputModeToggle?: (value: string) => void
  /** Fires whenever the highlighted row changes (wrap compatibility). */
  readonly onSelectionChange?: (value: T | undefined) => void
}

export function Select<T = string>({
  isDisabled = false,
  disableSelection = false,
  visibleOptionCount = 10,
  options,
  defaultValue,
  onCancel,
  onChange,
  onFocus,
  defaultFocusValue,
  layout = 'compact',
  inlineDescriptions = false,
  focusValue,
  isCancelable = true,
  onSelectionChange,
}: SelectProps<T>): React.ReactNode {
  const state = useSelectState<T>({
    visibleOptionCount,
    options,
    defaultValue: defaultFocusValue ?? defaultValue,
    onChange,
    onCancel,
    onFocus,
    focusValue,
  })

  useSelectInput<T>({
    isDisabled,
    state,
    disableSelection,
    isCancelable,
  })

  useEffect(() => {
    onSelectionChange?.(state.focusedValue)
  }, [state.focusedValue, onSelectionChange])

  const showUpArrow = state.visibleFromIndex > 0
  const showDownArrow = state.visibleToIndex < options.length

  return (
    <box flexDirection="column">
      {state.visibleOptions.map((opt, i) => {
        const isFocused = opt.index === (state.focusedIndex - 1)
        const isSelected = opt.value === state.value
        const isFirst = i === 0
        const isLast = i === state.visibleOptions.length - 1

        if ('type' in opt && opt.type === 'input') {
          return (
            <SelectInputOption
              key={String(opt.value)}
              label={opt.label}
              initialValue={opt.initialValue}
              placeholder={opt.placeholder}
              isActive={isFocused}
              allowEmptySubmitToCancel={opt.allowEmptySubmitToCancel}
              showLabelWithValue={opt.showLabelWithValue}
              labelValueSeparator={opt.labelValueSeparator}
              resetCursorOnUpdate={opt.resetCursorOnUpdate}
              onChange={value => {
                opt.onChange(value)
                state.onChange?.(opt.value)
              }}
              onCancel={() => {
                state.onCancel?.()
              }}
            />
          )
        }

        const description =
          layout === 'expanded' || !inlineDescriptions
            ? opt.description
            : opt.description
              ? `— ${opt.description}`
              : undefined

        return (
          <React.Fragment key={String(opt.value)}>
            <SelectOption
              isFocused={isFocused}
              isSelected={isSelected}
              description={description}
              shouldShowUpArrow={isFirst && showUpArrow}
              shouldShowDownArrow={isLast && showDownArrow}
            >
              {opt.label}
            </SelectOption>
            {layout === 'expanded' && !isLast && <text> </text>}
          </React.Fragment>
        )
      })}
      {options.length === 0 && (
        <text fg={c.dim}>(no options)</text>
      )}
    </box>
  )
}

import React, { type ReactNode } from 'react'
import { ListItem } from '../design-system/ListItem.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/customselect/select-option.tsx`.
 *
 * Thin wrapper around `<ListItem>` that maps the Ink scroll-arrow prop
 * names into the Lite component API. Exists primarily so `Select`
 * imports stay symmetric with upstream.
 */

export type SelectOptionProps = {
  readonly isFocused: boolean
  readonly isSelected: boolean
  readonly children: ReactNode
  readonly description?: string
  readonly shouldShowDownArrow?: boolean
  readonly shouldShowUpArrow?: boolean
  /** When false, the row trusts a nested input to own the cursor. */
  readonly declareCursor?: boolean
}

export function SelectOption({
  isFocused,
  isSelected,
  children,
  description,
  shouldShowDownArrow,
  shouldShowUpArrow,
  declareCursor,
}: SelectOptionProps) {
  return (
    <ListItem
      isFocused={isFocused}
      isSelected={isSelected}
      description={description}
      showScrollUp={shouldShowUpArrow}
      showScrollDown={shouldShowDownArrow}
      declareCursor={declareCursor}
      styled={false}
    >
      {children}
    </ListItem>
  )
}

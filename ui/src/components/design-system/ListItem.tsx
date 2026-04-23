import React, { type ReactNode } from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `ListItem` — a single row with focus
 * / selection affordances plus optional description and scroll arrows.
 * Used by `<Select>` and custom pickers.
 */

type Props = {
  isFocused?: boolean
  isSelected?: boolean
  description?: string
  dimDescription?: boolean
  showScrollUp?: boolean
  showScrollDown?: boolean
  /** When false, the component assumes a child declares its own
   *  cursor position (e.g. a nested text input). */
  declareCursor?: boolean
  /** When false, the row styles itself; otherwise the caller is
   *  expected to style its own children. */
  styled?: boolean
  /** Optional inline description rendered after the label. */
  inlineDescription?: boolean
  children: ReactNode
}

export function ListItem({
  isFocused = false,
  isSelected = false,
  description,
  dimDescription = true,
  showScrollUp = false,
  showScrollDown = false,
  inlineDescription = false,
  children,
}: Props) {
  const pointer = isFocused ? '\u276F' : ' '
  const marker = isSelected ? '\u25CF' : ' '

  const row = (
    <box flexDirection="row" gap={1}>
      <text fg={isFocused ? c.accent : c.dim}>{pointer}</text>
      <text fg={isSelected ? c.success : c.dim}>{marker}</text>
      <box flexDirection={inlineDescription ? 'row' : 'column'} gap={inlineDescription ? 1 : 0}>
        {isFocused ? (
          <strong>
            <text fg={c.textBright}>{children}</text>
          </strong>
        ) : (
          <text>{children}</text>
        )}
        {description && (
          <text fg={dimDescription ? c.dim : c.text}>
            {inlineDescription ? description : description}
          </text>
        )}
      </box>
    </box>
  )

  if (!showScrollUp && !showScrollDown) return row

  return (
    <box flexDirection="row" justifyContent="space-between" width="100%">
      {row}
      {(showScrollUp || showScrollDown) && (
        <text fg={c.dim}>
          {showScrollUp ? '\u2191' : ''}
          {showScrollDown ? '\u2193' : ''}
        </text>
      )}
    </box>
  )
}

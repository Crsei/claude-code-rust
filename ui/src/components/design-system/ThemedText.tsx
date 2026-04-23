import React, { type ReactNode } from 'react'
import { useTheme } from './ThemeProvider.js'
import { resolveColor, type ColorLike } from './color.js'

/**
 * Lite-native port of upstream's `ThemedText`. Thin wrapper around the
 * `<text>` primitive that accepts theme-key or raw-color strings and
 * applies `bold` / `dim` / `italic` via the matching markup tags.
 */

type Props = {
  children?: ReactNode
  color?: ColorLike
  bg?: ColorLike
  bold?: boolean
  dim?: boolean
  italic?: boolean
  underline?: boolean
  /** Truncation mode mirroring Ink's `wrap` prop. */
  wrap?: 'wrap' | 'truncate' | 'truncate-start' | 'truncate-middle' | 'truncate-end'
}

export function ThemedText({
  children,
  color,
  bg,
  bold = false,
  dim = false,
  italic = false,
  underline = false,
}: Props) {
  const theme = useTheme()
  const fg = dim ? resolveColor(color) ?? theme.dim : resolveColor(color)
  const bgColor = resolveColor(bg)

  let content: ReactNode = <text fg={fg} bg={bgColor}>{children}</text>
  if (bold) content = <strong>{content}</strong>
  if (italic) content = <em>{content}</em>
  if (underline) content = <span>{content}</span>
  return content
}

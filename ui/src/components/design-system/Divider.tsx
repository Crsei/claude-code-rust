import React from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of the upstream `Divider` (re-exported from
 * `@anthropic/ink` upstream). Renders a single-line horizontal rule
 * with configurable padding and character.
 */

type Props = {
  /** Horizontal padding on each side of the line. */
  padding?: number
  /** Character used for the rule. */
  char?: string
  /** Optional title embedded in the line: `── Title ──`. */
  title?: string
  /** Override rule color. Defaults to the muted theme colour. */
  color?: string
  /** Width in columns. Defaults to `100%`. */
  width?: number | 'auto' | `${number}%`
}

export function Divider({
  padding = 0,
  char = '\u2500',
  title,
  color,
  width = '100%',
}: Props) {
  const ruleColor = color ?? c.dim
  const line = char.repeat(8)
  if (title) {
    return (
      <box flexDirection="row" paddingX={padding} width={width}>
        <text fg={ruleColor}>{line} </text>
        <text>{title}</text>
        <text fg={ruleColor}> {line}</text>
      </box>
    )
  }
  return (
    <box paddingX={padding} width={width}>
      <text fg={ruleColor}>{char.repeat(60)}</text>
    </box>
  )
}

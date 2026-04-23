import React, { Children, Fragment, type ReactNode } from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `Byline`. Renders its children on a
 * single row with a middle-dot separator between each, suitable for
 * footer hints like `Enter confirm · Esc cancel · ↑/↓ select`.
 */

type Props = {
  children: ReactNode
  /** Separator glyph between children. */
  separator?: string
  /** Separator colour (defaults to dim). */
  separatorColor?: string
}

export function Byline({ children, separator = '\u00B7', separatorColor }: Props) {
  const items = Children.toArray(children).filter(
    child => child !== null && child !== undefined,
  )
  const sepColor = separatorColor ?? c.dim

  return (
    <box flexDirection="row" gap={1}>
      {items.map((child, i) => (
        <Fragment key={i}>
          {i > 0 && <text fg={sepColor}>{separator}</text>}
          {child}
        </Fragment>
      ))}
    </box>
  )
}

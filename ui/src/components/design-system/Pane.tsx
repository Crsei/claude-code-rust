import React, { type ReactNode } from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `Pane` — a semantic bordered
 * container used by permission dialogs and other overlay chromes. The
 * `color` prop maps to a theme-key that tints the border + title.
 */

export type PaneColor =
  | 'permission'
  | 'accent'
  | 'success'
  | 'error'
  | 'warning'
  | 'info'
  | 'background'

type Props = {
  children: ReactNode
  color?: PaneColor
  title?: string
  subtitle?: string
  borderStyle?: 'single' | 'double' | 'rounded' | 'heavy'
  paddingX?: number
  paddingY?: number
  width?: number | 'auto' | `${number}%`
}

const COLOR_MAP: Record<PaneColor, string> = {
  permission: c.warning,
  accent: c.accent,
  success: c.success,
  error: c.error,
  warning: c.warning,
  info: c.info,
  background: c.dim,
}

export function Pane({
  children,
  color = 'accent',
  title,
  subtitle,
  borderStyle = 'rounded',
  paddingX = 2,
  paddingY = 1,
  width,
}: Props) {
  const borderColor = COLOR_MAP[color]
  return (
    <box
      flexDirection="column"
      border
      borderStyle={borderStyle}
      borderColor={borderColor}
      paddingX={paddingX}
      paddingY={paddingY}
      width={width}
    >
      {title && (
        <strong>
          <text fg={borderColor}>{title}</text>
        </strong>
      )}
      {subtitle && <text fg={c.dim}>{subtitle}</text>}
      {(title || subtitle) && <text></text>}
      {children}
    </box>
  )
}

import React, { type ReactNode, useCallback } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'
import { Byline } from './Byline.js'
import { KeyboardShortcutHint } from './KeyboardShortcutHint.js'

/**
 * Lite-native port of upstream's `Dialog`. Renders a centred,
 * rounded-border modal with a title row, optional subtitle, a body,
 * and a default input-guide line (`Enter confirm · Esc cancel`).
 *
 * Esc triggers `onCancel`; the Dialog doesn't own Enter because most
 * dialogs compose their own confirm chrome (Select / buttons).
 */

type Props = {
  title?: ReactNode
  subtitle?: ReactNode
  /** Override the default input guide byline. Pass `null` to hide it. */
  inputGuide?: ReactNode | null
  /** Accent colour: maps `permission` -> warning, `background` -> dim. */
  color?: 'permission' | 'accent' | 'success' | 'error' | 'warning' | 'info' | 'background'
  /** When true, hides the border. */
  hideBorder?: boolean
  /** When true, hides the default input-guide byline. */
  hideInputGuide?: boolean
  onCancel?: () => void
  children: ReactNode
  width?: number | string
}

const COLOR_MAP = {
  permission: c.warning,
  accent: c.accent,
  success: c.success,
  error: c.error,
  warning: c.warning,
  info: c.info,
  background: c.dim,
} as const

export function Dialog({
  title,
  subtitle,
  inputGuide,
  color = 'accent',
  hideBorder = false,
  hideInputGuide = false,
  onCancel,
  children,
  width,
}: Props) {
  const borderColor = COLOR_MAP[color]

  const handleCancel = useCallback(() => {
    onCancel?.()
  }, [onCancel])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    if (event.name === 'escape') handleCancel()
  })

  const guide =
    inputGuide === null
      ? null
      : inputGuide !== undefined
        ? inputGuide
        : (
          <Byline>
            <KeyboardShortcutHint shortcut="Enter" action="confirm" />
            <KeyboardShortcutHint shortcut="Esc" action="cancel" />
          </Byline>
        )

  return (
    <box
      flexDirection="column"
      borderStyle={hideBorder ? undefined : 'rounded'}
      borderColor={hideBorder ? undefined : borderColor}
      paddingX={hideBorder ? 0 : 2}
      paddingY={hideBorder ? 0 : 1}
      width={width}
    >
      {title && (
        <strong>
          <text fg={borderColor}>{title}</text>
        </strong>
      )}
      {subtitle && <text fg={c.dim}>{subtitle}</text>}
      {(title || subtitle) && <text></text>}

      <box flexDirection="column">{children}</box>

      {!hideInputGuide && guide && (
        <box marginTop={1}>
          {guide}
        </box>
      )}
    </box>
  )
}

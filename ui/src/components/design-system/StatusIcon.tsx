import React from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's `StatusIcon` — a single glyph with a
 * theme-coloured foreground for success / error / warning / info /
 * pending / running states. Used throughout the dialog chrome.
 */

export type StatusIconKind =
  | 'success'
  | 'error'
  | 'warning'
  | 'info'
  | 'pending'
  | 'running'
  | 'dot'
  | 'check'
  | 'cross'

const GLYPHS: Record<StatusIconKind, string> = {
  success: '\u2713',
  error: '\u2717',
  warning: '\u26A0',
  info: '\u2139',
  pending: '\u25CB',
  running: '\u25C9',
  dot: '\u2022',
  check: '\u2713',
  cross: '\u2717',
}

const COLORS: Record<StatusIconKind, string> = {
  success: c.success,
  error: c.error,
  warning: c.warning,
  info: c.info,
  pending: c.dim,
  running: c.info,
  dot: c.text,
  check: c.success,
  cross: c.error,
}

type Props = {
  kind: StatusIconKind
  /** Override glyph colour. */
  color?: string
}

export function StatusIcon({ kind, color }: Props) {
  return <text fg={color ?? COLORS[kind]}>{GLYPHS[kind]}</text>
}

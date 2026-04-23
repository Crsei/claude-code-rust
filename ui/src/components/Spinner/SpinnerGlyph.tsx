import React from 'react'
import { c } from '../../theme.js'
import {
  getDefaultCharacters,
  interpolateColor,
  parseRGB,
  toRGBColor,
} from './utils.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/SpinnerGlyph.tsx`.
 *
 * Draws the single-cell glyph that bounces through the spinner frames.
 * Upstream uses Ink's `Box`/`Text` with a `keyof Theme` colour; this
 * port accepts the concrete hex string from `theme.c` so callers don't
 * need a theme provider.
 *
 * Three render paths mirror the upstream exactly:
 *  - `reducedMotion`: slow 2-second flashing dot.
 *  - `stalledIntensity > 0`: interpolate message colour toward error red.
 *  - Default: render the current spinner frame in the message colour.
 */

const DEFAULT_CHARACTERS = getDefaultCharacters()
const SPINNER_FRAMES = [
  ...DEFAULT_CHARACTERS,
  ...[...DEFAULT_CHARACTERS].reverse(),
]

const REDUCED_MOTION_DOT = '\u25CF'
const REDUCED_MOTION_CYCLE_MS = 2000
const ERROR_RED = { r: 171, g: 43, b: 63 }

type Props = {
  frame: number
  /** Hex colour string (e.g. `c.accent`). */
  messageColor: string
  /** 0\u20131; when > 0 interpolate toward red. */
  stalledIntensity?: number
  reducedMotion?: boolean
  /** Animation wall-clock ms — only consulted when `reducedMotion` is on. */
  time?: number
}

export function SpinnerGlyph({
  frame,
  messageColor,
  stalledIntensity = 0,
  reducedMotion = false,
  time = 0,
}: Props): React.ReactElement {
  if (reducedMotion) {
    const isDim =
      Math.floor(time / (REDUCED_MOTION_CYCLE_MS / 2)) % 2 === 1
    return (
      <box flexWrap="wrap" height={1} width={2}>
        <text fg={isDim ? c.dim : messageColor}>{REDUCED_MOTION_DOT}</text>
      </box>
    )
  }

  const spinnerChar = SPINNER_FRAMES[frame % SPINNER_FRAMES.length]

  if (stalledIntensity > 0) {
    const baseRGB = parseRGB(messageColor)
    if (baseRGB) {
      const interpolated = interpolateColor(baseRGB, ERROR_RED, stalledIntensity)
      return (
        <box flexWrap="wrap" height={1} width={2}>
          <text fg={toRGBColor(interpolated)}>{spinnerChar}</text>
        </box>
      )
    }
    const fg = stalledIntensity > 0.5 ? c.error : messageColor
    return (
      <box flexWrap="wrap" height={1} width={2}>
        <text fg={fg}>{spinnerChar}</text>
      </box>
    )
  }

  return (
    <box flexWrap="wrap" height={1} width={2}>
      <text fg={messageColor}>{spinnerChar}</text>
    </box>
  )
}

import React, { useMemo } from 'react'
import { c } from '../../theme.js'
import type { SpinnerMode } from './types.js'
import { interpolateColor, parseRGB, toRGBColor } from './utils.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/GlimmerMessage.tsx`.
 *
 * Renders the spinner's verb message with one of three animation styles:
 *  - `stalledIntensity > 0`: whole message interpolates to red.
 *  - `mode === 'tool-use'`: whole message pulses between `messageColor`
 *    and `shimmerColor` using `flashOpacity`.
 *  - Default (shimmer): a sliding 3-character band in `shimmerColor`
 *    slides across the message at 20fps; everything else stays in
 *    `messageColor`.
 *
 * The upstream uses `Intl.Segmenter` + `stringWidth` from Ink to split
 * along grapheme boundaries. OpenTUI doesn't ship `stringWidth`, and
 * the Rust port's messages are ASCII/emoji-free in practice, so we use
 * simple character indexing instead. If a grapheme-aware split becomes
 * necessary we can swap in the `Intl.Segmenter` path without changing
 * this component's props.
 */

const ERROR_RED = { r: 171, g: 43, b: 63 }

type Props = {
  message: string
  mode: SpinnerMode
  messageColor: string
  glimmerIndex: number
  flashOpacity: number
  shimmerColor: string
  stalledIntensity?: number
}

export function GlimmerMessage({
  message,
  mode,
  messageColor,
  glimmerIndex,
  flashOpacity,
  shimmerColor,
  stalledIntensity = 0,
}: Props): React.ReactElement | null {
  const messageWidth = useMemo(() => message.length, [message])

  if (!message) return null

  if (stalledIntensity > 0) {
    const baseRGB = parseRGB(messageColor)
    if (baseRGB) {
      const interpolated = interpolateColor(baseRGB, ERROR_RED, stalledIntensity)
      const color = toRGBColor(interpolated)
      return (
        <>
          <text fg={color}>{message}</text>
          <text fg={color}> </text>
        </>
      )
    }
    const color = stalledIntensity > 0.5 ? c.error : messageColor
    return (
      <>
        <text fg={color}>{message}</text>
        <text fg={color}> </text>
      </>
    )
  }

  if (mode === 'tool-use') {
    const baseRGB = parseRGB(messageColor)
    const shimmerRGB = parseRGB(shimmerColor)
    if (baseRGB && shimmerRGB) {
      const interpolated = interpolateColor(baseRGB, shimmerRGB, flashOpacity)
      return (
        <>
          <text fg={toRGBColor(interpolated)}>{message}</text>
          <text fg={messageColor}> </text>
        </>
      )
    }
    const color = flashOpacity > 0.5 ? shimmerColor : messageColor
    return (
      <>
        <text fg={color}>{message}</text>
        <text fg={messageColor}> </text>
      </>
    )
  }

  const shimmerStart = glimmerIndex - 1
  const shimmerEnd = glimmerIndex + 1

  if (shimmerStart >= messageWidth || shimmerEnd < 0) {
    return (
      <>
        <text fg={messageColor}>{message}</text>
        <text fg={messageColor}> </text>
      </>
    )
  }

  const clampedStart = Math.max(0, shimmerStart)
  const clampedEnd = Math.min(messageWidth - 1, shimmerEnd)
  const before = message.slice(0, clampedStart)
  const shim = message.slice(clampedStart, clampedEnd + 1)
  const after = message.slice(clampedEnd + 1)

  return (
    <>
      {before ? <text fg={messageColor}>{before}</text> : null}
      <text fg={shimmerColor}>{shim}</text>
      {after ? <text fg={messageColor}>{after}</text> : null}
      <text fg={messageColor}> </text>
    </>
  )
}

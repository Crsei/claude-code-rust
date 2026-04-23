import React from 'react'
import { interpolateColor, parseRGB, toRGBColor } from './utils.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/FlashingChar.tsx`.
 *
 * Renders a single character whose colour smoothly interpolates between
 * `messageColor` and `shimmerColor` according to `flashOpacity`
 * (0\u20131). Upstream took theme keys from Ink's `useTheme`; this port
 * takes the concrete hex strings directly so the component stays
 * theme-provider-free.
 */

type Props = {
  char: string
  flashOpacity: number
  messageColor: string
  shimmerColor: string
}

export function FlashingChar({
  char,
  flashOpacity,
  messageColor,
  shimmerColor,
}: Props): React.ReactElement {
  const baseRGB = parseRGB(messageColor)
  const shimmerRGB = parseRGB(shimmerColor)

  if (baseRGB && shimmerRGB) {
    const interpolated = interpolateColor(baseRGB, shimmerRGB, flashOpacity)
    return <text fg={toRGBColor(interpolated)}>{char}</text>
  }

  const shouldUseShimmer = flashOpacity > 0.5
  return (
    <text fg={shouldUseShimmer ? shimmerColor : messageColor}>{char}</text>
  )
}

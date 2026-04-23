import React from 'react'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/ShimmerChar.tsx`.
 *
 * Binary colour switch: a character is rendered in `shimmerColor` when
 * its index is within \u00b11 of the glimmer index, otherwise
 * `messageColor`. Matches the upstream behaviour byte-for-byte; the
 * only change is that the colours are concrete hex strings rather than
 * `keyof Theme` entries.
 */

type Props = {
  char: string
  index: number
  glimmerIndex: number
  messageColor: string
  shimmerColor: string
}

export function ShimmerChar({
  char,
  index,
  glimmerIndex,
  messageColor,
  shimmerColor,
}: Props): React.ReactElement {
  const isHighlighted = index === glimmerIndex
  const isNearHighlight = Math.abs(index - glimmerIndex) === 1
  const shouldUseShimmer = isHighlighted || isNearHighlight

  return (
    <text fg={shouldUseShimmer ? shimmerColor : messageColor}>{char}</text>
  )
}

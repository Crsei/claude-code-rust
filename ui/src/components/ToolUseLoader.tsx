import React, { useEffect, useState } from 'react'
import { c } from '../theme.js'

/**
 * Blinking indicator used next to running tool calls. Ports the upstream
 * `ToolUseLoader`
 * (`ui/examples/upstream-patterns/src/components/ToolUseLoader.tsx`) to
 * OpenTUI — `useBlink` becomes a local interval, Ink's `Box`/`Text` become
 * intrinsic `box`/`text` nodes, and the `BLACK_CIRCLE` figure stays as a
 * single unicode literal.
 *
 * - `isError` wins over `isUnresolved` for color selection.
 * - `shouldAnimate` gates both the blink and the alternating blank frame
 *   (stopped animations always show the circle so the status is stable).
 */

const BLACK_CIRCLE = '\u25CF'
const BLINK_INTERVAL_MS = 500

type Props = {
  isError: boolean
  isUnresolved: boolean
  shouldAnimate: boolean
}

export function ToolUseLoader({
  isError,
  isUnresolved,
  shouldAnimate,
}: Props): React.ReactElement {
  const [isBlinking, setIsBlinking] = useState(true)

  useEffect(() => {
    if (!shouldAnimate) {
      setIsBlinking(true)
      return
    }
    const id = setInterval(() => {
      setIsBlinking(b => !b)
    }, BLINK_INTERVAL_MS)
    return () => clearInterval(id)
  }, [shouldAnimate])

  const color = isUnresolved
    ? c.dim
    : isError
      ? c.error
      : c.success

  const showCircle =
    !shouldAnimate || isBlinking || isError || !isUnresolved

  return (
    <box minWidth={2}>
      <text fg={color}>{showCircle ? BLACK_CIRCLE : ' '}</text>
    </box>
  )
}

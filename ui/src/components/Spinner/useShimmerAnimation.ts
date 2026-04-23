import { useEffect, useRef, useState } from 'react'
import type { SpinnerMode } from './types.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/useShimmerAnimation.ts`.
 *
 * Upstream uses Ink's `useAnimationFrame(delay)` which produces a
 * `[ref, time]` tuple driven by a shared RAF clock. OpenTUI does not
 * expose the same viewport-aware hook, so this port runs a simple
 * `setInterval` at the requested delay and exposes `glimmerIndex`
 * directly. Returns 0 for `time` in the tuple so consumers that pass
 * `time` to other animation hooks keep working.
 *
 * When `isStalled` is true, the interval stops (matching upstream) and
 * the glimmer index is parked at -100 so the shimmer slot is offscreen.
 */

export function useShimmerAnimation(
  mode: SpinnerMode,
  message: string,
  isStalled: boolean,
): { glimmerIndex: number; time: number } {
  const glimmerSpeed = mode === 'requesting' ? 50 : 200
  const [time, setTime] = useState(0)
  const startRef = useRef(Date.now())
  const messageWidth = message.length

  useEffect(() => {
    if (isStalled) return
    startRef.current = Date.now()
    setTime(0)
    const id = setInterval(() => {
      setTime(Date.now() - startRef.current)
    }, glimmerSpeed)
    return () => clearInterval(id)
  }, [glimmerSpeed, isStalled])

  if (isStalled) {
    return { glimmerIndex: -100, time: 0 }
  }

  const cyclePosition = Math.floor(time / glimmerSpeed)
  const cycleLength = messageWidth + 20

  if (mode === 'requesting') {
    return {
      glimmerIndex: (cyclePosition % cycleLength) - 10,
      time,
    }
  }
  return {
    glimmerIndex: messageWidth + 10 - (cyclePosition % cycleLength),
    time,
  }
}

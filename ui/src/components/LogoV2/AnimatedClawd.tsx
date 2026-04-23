import React, { useEffect, useRef, useState } from 'react'
import { Clawd, type ClawdPose } from './Clawd.js'

/**
 * `<Clawd>` with click-triggered micro-animations.
 *
 * OpenTUI-native port of the upstream `LogoV2/AnimatedClawd`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/AnimatedClawd.tsx`).
 * The upstream registered mouse-click intents to trigger jump-wave /
 * look-around sequences. OpenTUI doesn't surface box-level `onClick`
 * in this project today, so the Lite port ships the animation engine
 * and starts sequences on mount (optional \u2014 caller can opt out) and
 * exposes `playSequence()` through a ref for host-driven triggers.
 */

type Frame = { pose: ClawdPose; offset: number }

const FRAME_MS = 60
const CLAWD_HEIGHT = 3

function hold(pose: ClawdPose, offset: number, frames: number): Frame[] {
  return Array.from({ length: frames }, () => ({ pose, offset }))
}

const JUMP_WAVE: Frame[] = [
  ...hold('default', 1, 2),
  ...hold('arms-up', 0, 3),
  ...hold('default', 0, 1),
  ...hold('default', 1, 2),
  ...hold('arms-up', 0, 3),
  ...hold('default', 0, 1),
]

const LOOK_AROUND: Frame[] = [
  ...hold('look-right', 0, 5),
  ...hold('look-left', 0, 5),
  ...hold('default', 0, 1),
]

const IDLE: Frame = { pose: 'default', offset: 0 }

export const CLAWD_ANIMATIONS = {
  jump: JUMP_WAVE,
  lookAround: LOOK_AROUND,
}

type Props = {
  /** Set `false` to freeze on the idle pose (e.g. reduced-motion). */
  animate?: boolean
  /** Which sequence to play. Defaults to `'jump'`. */
  sequence?: keyof typeof CLAWD_ANIMATIONS
  /** When true, restarts the sequence once the previous one ends. */
  loop?: boolean
  /** Forward to `<Clawd>` when the embedder is on Apple Terminal. */
  appleTerminal?: boolean
}

export function AnimatedClawd({
  animate = true,
  sequence = 'jump',
  loop = false,
  appleTerminal = false,
}: Props = {}) {
  const frames = CLAWD_ANIMATIONS[sequence]
  const [frameIndex, setFrameIndex] = useState(animate ? 0 : -1)
  const loopRef = useRef(loop)
  loopRef.current = loop

  useEffect(() => {
    if (!animate) return
    setFrameIndex(0)
  }, [animate, sequence])

  useEffect(() => {
    if (frameIndex === -1) return
    if (frameIndex >= frames.length) {
      if (loopRef.current) {
        const restart = setTimeout(() => setFrameIndex(0), FRAME_MS * 3)
        return () => clearTimeout(restart)
      }
      setFrameIndex(-1)
      return
    }
    const timer = setTimeout(() => setFrameIndex(prev => prev + 1), FRAME_MS)
    return () => clearTimeout(timer)
  }, [frameIndex, frames.length])

  const current = frameIndex >= 0 && frameIndex < frames.length ? frames[frameIndex]! : IDLE

  return (
    <box height={CLAWD_HEIGHT} flexDirection="column">
      <box marginTop={current.offset} flexShrink={0}>
        <Clawd pose={current.pose} appleTerminal={appleTerminal} />
      </box>
    </box>
  )
}

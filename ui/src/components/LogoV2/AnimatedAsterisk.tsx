import React, { useEffect, useRef, useState } from 'react'

/**
 * Color-sweeping teardrop-asterisk glyph used by the notice components
 * (`VoiceModeNotice`, `Opus1mMergeNotice`).
 *
 * OpenTUI-native port of the upstream `LogoV2/AnimatedAsterisk`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/AnimatedAsterisk.tsx`).
 * Upstream drove the hue cycle with Ink's `useAnimationFrame` and
 * exposed a settings-driven `prefersReducedMotion` opt-out. The Lite
 * port replaces the animation frame source with a 50 ms `setInterval`
 * and accepts the reduced-motion preference as a prop so it stays
 * decoupled from the settings stack.
 */

const SWEEP_DURATION_MS = 1500
const SWEEP_COUNT = 2
const TOTAL_ANIMATION_MS = SWEEP_DURATION_MS * SWEEP_COUNT
const SETTLED_COLOR = '#999999'
const TEARDROP_ASTERISK = '\u273B'

type Props = {
  char?: string
  reducedMotion?: boolean
}

function hueToRgb(hue: number): string {
  const h = hue / 60
  const sector = Math.floor(h) % 6
  const f = h - Math.floor(h)
  const q = Math.round((1 - f) * 255)
  const t = Math.round(f * 255)
  const components: Record<number, [number, number, number]> = {
    0: [255, t, 0],
    1: [q, 255, 0],
    2: [0, 255, t],
    3: [0, q, 255],
    4: [t, 0, 255],
    5: [255, 0, q],
  }
  const [r, g, b] = components[sector] ?? [255, 255, 255]
  const toHex = (value: number) => value.toString(16).padStart(2, '0')
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`
}

export function AnimatedAsterisk({
  char = TEARDROP_ASTERISK,
  reducedMotion = false,
}: Props = {}) {
  const [done, setDone] = useState(reducedMotion)
  const [now, setNow] = useState(0)
  const startRef = useRef<number | null>(null)

  useEffect(() => {
    if (done) return
    const begin = Date.now()
    startRef.current = begin
    const tick = setInterval(() => setNow(Date.now() - begin), 50)
    const end = setTimeout(() => {
      clearInterval(tick)
      setDone(true)
    }, TOTAL_ANIMATION_MS)
    return () => {
      clearInterval(tick)
      clearTimeout(end)
    }
  }, [done])

  if (done) {
    return <text fg={SETTLED_COLOR}>{char}</text>
  }

  const hue = ((now / SWEEP_DURATION_MS) * 360) % 360
  const fg = hueToRgb(hue)
  return <text fg={fg}>{char}</text>
}

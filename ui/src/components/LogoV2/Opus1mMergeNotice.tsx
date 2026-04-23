import React from 'react'
import { c } from '../../theme.js'
import { AnimatedAsterisk } from './AnimatedAsterisk.js'

/**
 * "Opus now defaults to 1M context" promotional line.
 *
 * OpenTUI-native port of the upstream `LogoV2/Opus1mMergeNotice`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/Opus1mMergeNotice.tsx`).
 * Upstream read `isOpus1mMergeEnabled()` + a per-user impression count
 * (max 6) from global config. The Lite port accepts `visible` so the
 * Rust backend keeps authority over eligibility and impression tracking.
 */

const UP_ARROW = '\u2191'

type Props = {
  visible?: boolean
  reducedMotion?: boolean
}

export function Opus1mMergeNotice({
  visible = false,
  reducedMotion,
}: Props = {}) {
  if (!visible) return null
  return (
    <box paddingLeft={2} flexDirection="row">
      <AnimatedAsterisk char={UP_ARROW} reducedMotion={reducedMotion} />
      <text fg={c.dim}>
        {' Opus now defaults to 1M context \u00B7 5x more room, same pricing'}
      </text>
    </box>
  )
}

/**
 * Mirror of the upstream `shouldShowOpus1mMergeNotice()` helper \u2014
 * the Lite port keeps it as a pure function the backend can call.
 */
export function shouldShowOpus1mMergeNotice(
  opts: { enabled: boolean; seenCount: number } = { enabled: false, seenCount: 0 },
): boolean {
  const MAX_SHOW_COUNT = 6
  return opts.enabled && opts.seenCount < MAX_SHOW_COUNT
}

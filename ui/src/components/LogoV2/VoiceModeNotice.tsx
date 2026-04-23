import React from 'react'
import { c } from '../../theme.js'
import { AnimatedAsterisk } from './AnimatedAsterisk.js'

/**
 * "Voice mode is now available" promotional line.
 *
 * OpenTUI-native port of the upstream `LogoV2/VoiceModeNotice`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/VoiceModeNotice.tsx`).
 * Upstream was gated behind the `VOICE_MODE` bundle feature and tracked
 * an impression count in global config (max 3). The Lite port exposes
 * a `visible` prop so the Rust backend / host decides whether to show
 * it \u2014 eligibility and impression accounting live server-side.
 */

type Props = {
  visible?: boolean
  reducedMotion?: boolean
}

export function VoiceModeNotice({ visible = false, reducedMotion }: Props = {}) {
  if (!visible) return null
  return (
    <box paddingLeft={2} flexDirection="row">
      <AnimatedAsterisk reducedMotion={reducedMotion} />
      <text fg={c.dim}> Voice mode is now available \u00B7 /voice to enable</text>
    </box>
  )
}

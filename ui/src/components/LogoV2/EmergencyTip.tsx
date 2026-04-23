import React from 'react'
import { c } from '../../theme.js'

/**
 * "Tip of the feed" one-liner shown at the top of the home screen.
 *
 * OpenTUI-native port of the upstream `LogoV2/EmergencyTip`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/EmergencyTip.tsx`).
 * Upstream fetched the tip from GrowthBook dynamic config
 * (`tengu-top-of-feed-tip`) and tracked a per-user "last shown" marker
 * in global config. The Lite port takes the tip via props; the Rust
 * backend decides when / whether to supply one and when to stop.
 */

export type EmergencyTipColor = 'dim' | 'warning' | 'error'

type Props = {
  tip?: string | null
  color?: EmergencyTipColor
}

function tipFg(color: EmergencyTipColor | undefined): string | undefined {
  switch (color) {
    case 'warning':
      return c.warning
    case 'error':
      return c.error
    case 'dim':
    default:
      return c.dim
  }
}

export function EmergencyTip({ tip, color = 'dim' }: Props) {
  if (!tip) return null
  return (
    <box paddingLeft={2} flexDirection="column">
      <text fg={tipFg(color)}>{tip}</text>
    </box>
  )
}

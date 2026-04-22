import React from 'react'
import { c } from '../../theme.js'

/**
 * Inline status hint shown to the right of the composer while the
 * backend is busy. Surfaces the reasoning / thinking phase plus
 * elapsed seconds.
 *
 * Lite-native counterpart of the sample tree's `ThinkingToggle`
 * (`ui/examples/upstream-patterns/src/components/ThinkingToggle.tsx`)
 * — we only surface the label here because the active frontend
 * forwards phase transitions as store flags, not interactive toggles.
 */

type Props = {
  /** Prepared label, e.g. `"reasoning 3s"` or `""` when idle. */
  workedTag: string
}

export function ModeIndicator({ workedTag }: Props) {
  if (!workedTag) return null
  return <text fg={c.dim}>  * {workedTag}</text>
}

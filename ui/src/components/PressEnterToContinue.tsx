import React from 'react'
import { c } from '../theme.js'

/**
 * Small prompt shown at the bottom of dialogs waiting on a user confirmation.
 * Lite-native port of the sample tree's `PressEnterToContinue`
 * (`ui/examples/upstream-patterns/src/components/PressEnterToContinue.tsx`).
 */
export function PressEnterToContinue({ label = 'continue' }: { label?: string }) {
  return (
    <text fg={c.warning}>
      Press <strong>Enter</strong> to {label}…
    </text>
  )
}

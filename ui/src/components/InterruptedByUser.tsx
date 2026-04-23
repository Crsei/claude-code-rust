import React from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream `InterruptedByUser`
 * (`ui/examples/upstream-patterns/src/components/InterruptedByUser.tsx`).
 *
 * Upstream branches on `process.env.USER_TYPE === 'ant'` for an internal
 * `/issue` hint; cc-rust has no internal-user channel so we keep only the
 * public variant.
 */
export function InterruptedByUser() {
  return (
    <text fg={c.dim}>
      Interrupted <span>·</span> What should Claude do instead?
    </text>
  )
}

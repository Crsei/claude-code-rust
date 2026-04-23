import React from 'react'

/**
 * Internal-only component. Upstream shows experiment enrollment status
 * for `USER_TYPE === 'ant'` builds; the Lite build doesn't track
 * experiments frontend-side, so this is a `null`-returning stub kept
 * so the LogoV2 shell can import it unconditionally.
 */
export function ExperimentEnrollmentNotice(): React.ReactNode {
  return null
}

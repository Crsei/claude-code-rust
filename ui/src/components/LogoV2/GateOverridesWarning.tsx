import React from 'react'

/**
 * Internal-only component. Upstream displays a warning when feature-gate
 * overrides (`CLAUDE_INTERNAL_FC_OVERRIDES`) are active. Stubbed in the
 * Lite build \u2014 the Rust backend doesn't ship the feature-gate override
 * plumbing, so this always renders `null`.
 *
 * Kept as a named export so the LogoV2 shell can import it
 * unconditionally without a `USER_TYPE` branch.
 */
export function GateOverridesWarning(): React.ReactNode {
  return null
}

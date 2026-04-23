import React from 'react'
import { c } from '../../theme.js'

/**
 * Compact guest-passes upsell row rendered inside the condensed logo.
 *
 * OpenTUI-native port of the upstream `LogoV2/GuestPassesUpsell`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/GuestPassesUpsell.tsx`).
 * Upstream exposed `useShowGuestPassesUpsell()` and
 * `incrementGuestPassesSeenCount()` helpers that talked directly to
 * global config and the referral cache. The Lite port leaves eligibility
 * and impression-count side effects Rust-side and renders only when the
 * caller hands it a `visible` prop.
 */

type Props = {
  visible?: boolean
  /** Referrer reward copy, e.g. "$10" or `null` if not eligible yet. */
  reward?: string | null
}

export function GuestPassesUpsell({ visible = true, reward }: Props = {}) {
  if (!visible) return null
  const suffix = reward
    ? `Share Claude Code and earn ${reward} of extra usage \u00B7 /passes`
    : '3 guest passes at /passes'
  return (
    <text fg={c.dim}>
      <span fg={c.accent}>[\u273B]</span> <span fg={c.accent}>[\u273B]</span>{' '}
      <span fg={c.accent}>[\u273B]</span> \u00B7 {suffix}
    </text>
  )
}

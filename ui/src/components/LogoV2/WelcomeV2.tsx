import React from 'react'
import { c } from '../../theme.js'

/**
 * Decorative "Welcome to Claude Code" banner shown before the user has
 * any messages. Upstream hand-drew two ASCII variants (light/dark) plus
 * an Apple-Terminal specialised render.
 *
 * OpenTUI-native port of the upstream `LogoV2/WelcomeV2`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/WelcomeV2.tsx`).
 * The pixel-perfect banner relies on Ink's `useTheme` + two dozen manually
 * positioned `<Text>` rows. The Lite port keeps the textual banner \u2014 same
 * widths so downstream layout math still works \u2014 and drops the per-theme
 * variants (the OpenTUI palette has a single `c.accent` shade).
 */

const WELCOME_V2_WIDTH = 58
const CLAWD_BODY = '#CC6B2D'
const CLAWD_BACKGROUND = '#FFE3D0'

type Props = {
  version?: string
}

export function WelcomeV2({ version }: Props = {}) {
  const welcomeMessage = 'Welcome to Claude Code'
  return (
    <box width={WELCOME_V2_WIDTH} flexDirection="column">
      <text>
        <span fg={c.accent}>{welcomeMessage} </span>
        {version && <span fg={c.dim}>v{version}</span>}
      </text>
      <text fg={c.dim}>
        {'\u2026'.repeat(WELCOME_V2_WIDTH)}
      </text>
      <text>{' '.repeat(WELCOME_V2_WIDTH)}</text>
      <text>{'     *                                       \u2588\u2588\u2588\u2588\u2588\u2593\u2593\u2591     '}</text>
      <text>{'                                 *         \u2588\u2588\u2588\u2593\u2591     \u2591\u2591   '}</text>
      <text>{'            \u2591\u2591\u2591\u2591\u2591\u2591                        \u2588\u2588\u2588\u2593\u2591           '}</text>
      <text>{'    \u2591\u2591\u2591   \u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591                      \u2588\u2588\u2588\u2593\u2591           '}</text>
      <text>
        {'   \u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591\u2591    '}
        <strong>*</strong>
        {'                \u2588\u2588\u2593\u2591\u2591      \u2593   '}
      </text>
      <text>{'                                             \u2591\u2593\u2593\u2588\u2588\u2588\u2593\u2593\u2591    '}</text>
      <text>
        {'      '}
        <span fg={CLAWD_BODY}> \u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588 </span>
        {'                                       '}
        <span fg={c.dim}>*</span>
      </text>
      <text>
        {'      '}
        <span fg={CLAWD_BODY} bg={CLAWD_BACKGROUND}>\u2588\u2588\u2584\u2588\u2588\u2588\u2588\u2588\u2584\u2588\u2588</span>
        <strong>{'                        *               '}</strong>
      </text>
      <text>
        {'      '}
        <span fg={CLAWD_BODY}> \u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588\u2588 </span>
        {'     *                                   '}
      </text>
      <text fg={c.dim}>
        {'\u2026\u2026\u2026\u2026\u2026\u2026\u2026'}
        <span fg={CLAWD_BODY}>\u2588 \u2588   \u2588 \u2588</span>
        {'\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026\u2026'}
      </text>
    </box>
  )
}

import React from 'react'

/**
 * "Clawd" ASCII mascot, shown inside the LogoV2 frame.
 *
 * OpenTUI-native port of the upstream `LogoV2/Clawd`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/Clawd.tsx`).
 * Upstream used two themed Ink colors (`clawd_body`, `clawd_background`)
 * and selectively fell back to a background-fill rendering on Apple
 * Terminal because Terminal.app adds line spacing between regular
 * characters but not between background runs. The Lite port keeps the
 * same pose fragments and the Apple-Terminal alternate; colors are
 * pinned to OpenTUI theme slots.
 */

export type ClawdPose = 'default' | 'arms-up' | 'look-left' | 'look-right'

type Props = {
  pose?: ClawdPose
  /** Override for the "Apple Terminal" alternate render \u2014 useful when
   *  the embedder detects a terminal where per-cell line spacing would
   *  break the standard render. Defaults to `false`. */
  appleTerminal?: boolean
}

type Segments = {
  r1L: string
  r1E: string
  r1R: string
  r2L: string
  r2R: string
}

const POSES: Record<ClawdPose, Segments> = {
  default: { r1L: ' \u2590', r1E: '\u259B\u2588\u2588\u2588\u259C', r1R: '\u258C', r2L: '\u259D\u259C', r2R: '\u259B\u2598' },
  'look-left': { r1L: ' \u2590', r1E: '\u259F\u2588\u2588\u2588\u259F', r1R: '\u258C', r2L: '\u259D\u259C', r2R: '\u259B\u2598' },
  'look-right': { r1L: ' \u2590', r1E: '\u2599\u2588\u2588\u2588\u2599', r1R: '\u258C', r2L: '\u259D\u259C', r2R: '\u259B\u2598' },
  'arms-up': { r1L: '\u2597\u259F', r1E: '\u259B\u2588\u2588\u2588\u259C', r1R: '\u2599\u2596', r2L: ' \u259C', r2R: '\u259B ' },
}

const APPLE_EYES: Record<ClawdPose, string> = {
  default: ' \u2597   \u2596 ',
  'look-left': ' \u2598   \u2598 ',
  'look-right': ' \u259D   \u259D ',
  'arms-up': ' \u2597   \u2596 ',
}

const BODY = '#CC6B2D'
const BACKGROUND = '#FFE3D0'

export function Clawd({ pose = 'default', appleTerminal = false }: Props = {}) {
  if (appleTerminal) {
    return (
      <box flexDirection="column" alignItems="center">
        <text>
          <span fg={BODY}>\u2597</span>
          <span fg={BACKGROUND} bg={BODY}>{APPLE_EYES[pose]}</span>
          <span fg={BODY}>\u2596</span>
        </text>
        <text bg={BODY}>{'       '}</text>
        <text fg={BODY}>{'\u2598\u2598 \u259D\u259D'}</text>
      </box>
    )
  }

  const p = POSES[pose]
  return (
    <box flexDirection="column">
      <text>
        <span fg={BODY}>{p.r1L}</span>
        <span fg={BODY} bg={BACKGROUND}>{p.r1E}</span>
        <span fg={BODY}>{p.r1R}</span>
      </text>
      <text>
        <span fg={BODY}>{p.r2L}</span>
        <span fg={BODY} bg={BACKGROUND}>{'\u2588\u2588\u2588\u2588\u2588'}</span>
        <span fg={BODY}>{p.r2R}</span>
      </text>
      <text fg={BODY}>{'  \u2598\u2598 \u259D\u259D  '}</text>
    </box>
  )
}

/** Exposed so downstream animators can reuse the theme slots if they
 *  build their own palette. */
export const CLAWD_COLORS = {
  body: BODY,
  background: BACKGROUND,
}

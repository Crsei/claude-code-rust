/**
 * Effort-level symbol helper. Lite-native re-host of
 * `ui/examples/upstream-patterns/src/components/EffortIndicator.ts` that
 * drops the `modelSupportsEffort` / `getDisplayedEffortLevel` helpers
 * (upstream config lookup) and keeps the two primitives components need:
 * `effortLevelToSymbol` for rendering and `getEffortNotificationText` for
 * the `/effort` notification.
 *
 * Symbols mirror the upstream `constants/figures` table. They are
 * hand-picked so every level renders in a monospaced cell on the
 * terminals cc-rust targets (Windows Terminal / iTerm / VSCode integrated).
 */

export type EffortLevel = 'low' | 'medium' | 'high' | 'max'

export type EffortValue = EffortLevel | 'minimal' | string | undefined

const EFFORT_LOW = '\u25D4' // ◔
const EFFORT_MEDIUM = '\u25D0' // ◐
const EFFORT_HIGH = '\u25D5' // ◕
const EFFORT_MAX = '\u25CF' // ●

export function effortLevelToSymbol(level: EffortLevel | string | undefined): string {
  switch (level) {
    case 'low':
      return EFFORT_LOW
    case 'medium':
      return EFFORT_MEDIUM
    case 'high':
      return EFFORT_HIGH
    case 'max':
      return EFFORT_MAX
    default:
      // Defensive: level can originate from remote config. If an unknown
      // value slips through, render the high symbol rather than undefined.
      return EFFORT_HIGH
  }
}

/**
 * Build the text for the effort-changed notification, e.g.
 * "◐ medium · /effort". Returns `undefined` when the model does not
 * support effort selection — the caller should skip the notification
 * entirely in that case.
 *
 * cc-rust does not yet expose a model → effort-support table over IPC,
 * so callers currently pass a hard-coded allow-list (Opus / Sonnet
 * tiers). When the backend starts forwarding that information we can
 * add a `modelSupportsEffort(model)` helper here, matching upstream.
 */
export function getEffortNotificationText(
  effortValue: EffortValue,
  model: string,
): string | undefined {
  if (!model) return undefined
  const level = normalizeEffortLevel(effortValue)
  if (!level) return undefined
  return `${effortLevelToSymbol(level)} ${level} · /effort`
}

/** Narrow an arbitrary effort string down to a known level, or `undefined`. */
export function normalizeEffortLevel(value: EffortValue): EffortLevel | undefined {
  switch (value) {
    case 'low':
    case 'medium':
    case 'high':
    case 'max':
      return value
    case 'minimal':
      return 'low'
    default:
      return undefined
  }
}

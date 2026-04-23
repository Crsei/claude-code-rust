/**
 * Shared types for the Spinner suite. Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/types.ts`.
 */

/**
 * Animation flavour. `requesting` runs the glimmer left-to-right (same
 * direction as text), everything else runs it right-to-left.
 */
export type SpinnerMode =
  | 'requesting'
  | 'responding'
  | 'thinking'
  | 'tool-input'
  | 'tool-use'

/** RGB tuple used by the colour interpolation helpers. */
export type RGBColor = {
  r: number
  g: number
  b: number
}

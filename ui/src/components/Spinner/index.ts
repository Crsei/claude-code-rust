/**
 * Barrel for the Spinner suite. Mirrors the upstream layout
 * (`ui/examples/upstream-patterns/src/components/Spinner/index.ts`) — the
 * simple single-row `Spinner` stays at `../Spinner.tsx` and is re-exported
 * here, while the advanced shimmer / glimmer / teammate-tree components
 * live in this folder as their upstream files do.
 *
 * Teammate components are exported eagerly here (dead-code elimination
 * in bundlers works differently for OpenTUI, and the Rust port's REPL
 * doesn't spin up a teammate tree by default, so the cost is trivial).
 */
export { FlashingChar } from './FlashingChar.js'
export { GlimmerMessage } from './GlimmerMessage.js'
export { ShimmerChar } from './ShimmerChar.js'
export { SpinnerGlyph } from './SpinnerGlyph.js'
export { TeammateSpinnerLine } from './TeammateSpinnerLine.js'
export { TeammateSpinnerTree } from './TeammateSpinnerTree.js'
export { TEAMMATE_SELECT_HINT } from './teammateSelectHint.js'
export { useShimmerAnimation } from './useShimmerAnimation.js'
export { useStalledAnimation } from './useStalledAnimation.js'
export type { SpinnerMode, RGBColor } from './types.js'
export {
  getDefaultCharacters,
  hueToRgb,
  interpolateColor,
  parseRGB,
  toRGBColor,
} from './utils.js'
// Re-export the simple flat Spinner for callers that just want a
// single-row "Working\u2026" indicator without the animation stack.
export { Spinner } from '../Spinner.js'

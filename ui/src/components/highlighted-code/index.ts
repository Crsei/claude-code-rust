/**
 * Barrel for the `highlighted-code` folder. Mirrors the upstream
 * layout at `ui/examples/upstream-patterns/src/components/highlighted-code/`.
 *
 * Until a full OpenTUI-native syntax highlighter lands, the top-level
 * `HighlightedCode` surface is supplied by `components/HighlightedCode.tsx`
 * (see the Rust-side highlighter wiring). The fallback renderer lives
 * here so both the top-level and future sub-surfaces can reuse it.
 */
export {
  HighlightedCodeFallback,
  convertLeadingTabsToSpaces,
} from './Fallback.js'

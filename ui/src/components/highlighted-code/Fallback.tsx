import React from 'react'
import { extname } from 'path'
import { c } from '../../theme.js'

/**
 * No-syntax-highlighter fallback renderer for code blocks.
 *
 * OpenTUI-native port of the upstream `highlighted-code/Fallback`
 * (`ui/examples/upstream-patterns/src/components/highlighted-code/Fallback.tsx`).
 * Upstream streamed the code through `getCliHighlightPromise()` and an
 * ANSI-capable `<Ansi>` node; the Lite port is OpenTUI-native — OpenTUI
 * `<text>` nodes do not interpret embedded ANSI escapes, so we render
 * the raw source with leading tabs converted to spaces and let the
 * caller pick up syntax coloring from a higher-level surface once a
 * highlighter is wired in.
 *
 * Upstream's "hl.highlight()" cache was a hot-path optimization for
 * virtual-scroll remounts. With the OpenTUI diff renderer the cost
 * profile is different and a plain render is enough for the fallback
 * path. If a highlighter lands, re-introduce caching keyed by
 * `hashPair(language, code)` as upstream did.
 */

type Props = {
  code: string
  filePath: string
  dim?: boolean
  /** Skip the language detection entirely \u2014 mirrors upstream prop. */
  skipColoring?: boolean
}

/** Replace leading tabs in every line with two spaces each. */
export function convertLeadingTabsToSpaces(code: string): string {
  return code.replace(/^\t+/gm, m => '  '.repeat(m.length))
}

export function HighlightedCodeFallback({
  code,
  filePath,
  dim = false,
  skipColoring: _skipColoring = false,
}: Props) {
  const rendered = convertLeadingTabsToSpaces(code)
  // `filePath` is retained so consumers that later gain a syntax
  // highlighter can derive the language without changing the call site.
  void filePath
  void extname
  const fg = dim ? c.dim : undefined
  return <text fg={fg}>{rendered}</text>
}

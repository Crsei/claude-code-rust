import React from 'react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/HighlightedCode.tsx`.
 *
 * Upstream drives a Rust NAPI colouriser (tree-sitter) to render code
 * blocks with theme-aware syntax highlighting, plus a 1-indexed line
 * gutter and ANSI splitting. cc-rust does not ship the colouriser, so
 * we render the plain text through OpenTUI's `<text>` with the Lite
 * theme and keep the line gutter when width allows. Downstream
 * components (`FileEditToolUseRejectedMessage`, future `Markdown` code
 * blocks) consume the same props signature as upstream so the
 * integration site does not need to know which implementation is
 * active.
 *
 * The `dim` prop forwards the "muted this block" intent used by the
 * rejected-write renderer. `width` is accepted for signature
 * compatibility but the OpenTUI `<text>` layer wraps by its enclosing
 * box rather than an explicit width, so we only use it to decide
 * whether to draw the gutter (narrow cells hide it to stay readable).
 */

type Props = {
  code: string
  filePath?: string
  /** Width hint from the caller. Narrow cells disable the gutter. */
  width?: number
  /** When true, render the block in the muted/dim palette. */
  dim?: boolean
  /**
   * When true, skip the line gutter even if the width allows it — used
   * by tests and code-only callers (Markdown code fences).
   */
  hideGutter?: boolean
}

const MIN_GUTTER_WIDTH = 30

function digitsIn(n: number): number {
  if (n < 10) return 1
  return Math.floor(Math.log10(n)) + 1
}

function padLeft(n: number, width: number): string {
  const s = String(n)
  return s.length >= width ? s : ' '.repeat(width - s.length) + s
}

export function HighlightedCode({
  code,
  filePath,
  width,
  dim = false,
  hideGutter = false,
}: Props) {
  const lines = code.length === 0 ? [''] : code.split('\n')
  const showGutter =
    !hideGutter && (width === undefined || width >= MIN_GUTTER_WIDTH) && lines.length > 1
  const gutterWidth = showGutter ? digitsIn(lines.length) : 0
  const bodyColor = dim ? c.dim : c.text

  return (
    <box flexDirection="column" width="100%">
      {filePath && (
        <text fg={c.dim}>
          <em>{filePath}</em>
        </text>
      )}
      {lines.map((line, i) => (
        <box key={i} flexDirection="row">
          {showGutter && (
            <text fg={c.dim}>
              {padLeft(i + 1, gutterWidth)}
              {'  '}
            </text>
          )}
          <text fg={bodyColor} selectable>
            {line.length === 0 ? ' ' : line}
          </text>
        </box>
      ))}
    </box>
  )
}

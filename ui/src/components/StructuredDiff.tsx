import React from 'react'
import { c } from '../theme.js'
import {
  hunkGutterWidth,
  type DiffHunk,
  type DiffLine,
} from './StructuredDiff/hunks.js'

/**
 * Render a single unified-diff hunk with theme colours and a line-number
 * gutter. Lite-native replacement for the sample tree's `StructuredDiff`
 * (`ui/examples/upstream-patterns/src/components/StructuredDiff.tsx`) that
 * drops the Rust NAPI colouriser and the `diff` runtime dependency.
 *
 * Renders a hunk with a header row (`@@ -a,b +c,d @@`) followed by
 * marker-tagged lines. Context lines are dimmed; adds are green; removes
 * are red. Use `<StructuredDiffList>` for lists of hunks separated by
 * ellipses — this component handles exactly one hunk, matching the
 * upstream split.
 */

type Props = {
  hunk: DiffHunk
  /** When set, show at most this many lines (context + edits combined) so
   *  long hunks don't overflow the bubble. */
  maxLinesPerHunk?: number
  /** When true, swallow the hunk header row. Used by callers that draw
   *  their own frame (e.g. the permission preview). */
  hideHeader?: boolean
  /** Ignored — accepted for API parity with the upstream renderer's NAPI
   *  code path. The Lite renderer never depends on the terminal width. */
  width?: number
  /** Ignored — accepted for API parity with the upstream renderer. The
   *  Lite renderer doesn't draw the whole hunk in a dim tone; context
   *  rows are already dimmed. */
  dim?: boolean
  /** Ignored — the upstream signature carries these for the NAPI syntax
   *  highlighter. Accepted so the shape stays drop-in compatible. */
  filePath?: string
  firstLine?: string | null
  fileContent?: string
  skipHighlighting?: boolean
}

const MARKER: Record<DiffLine['kind'], string> = {
  context: ' ',
  add: '+',
  remove: '-',
}

const COLOR: Record<DiffLine['kind'], string> = {
  context: c.dim,
  add: c.success,
  remove: c.error,
}

function padLeft(s: string, width: number): string {
  if (s.length >= width) return s
  return ' '.repeat(width - s.length) + s
}

export function StructuredDiff({
  hunk,
  maxLinesPerHunk,
  hideHeader,
}: Props): React.ReactElement {
  const gutter = hunkGutterWidth(hunk)
  let oldLine = hunk.oldStart
  let newLine = hunk.newStart
  const truncated =
    typeof maxLinesPerHunk === 'number' && hunk.lines.length > maxLinesPerHunk
  const visible = truncated
    ? hunk.lines.slice(0, maxLinesPerHunk)
    : hunk.lines
  const hiddenCount = truncated ? hunk.lines.length - visible.length : 0

  return (
    <box flexDirection="column">
      {!hideHeader && (
        <text fg={c.info}>
          @@ -{hunk.oldStart},{hunk.oldLines} +{hunk.newStart},{hunk.newLines} @@
        </text>
      )}
      {visible.map((line, index) => {
        const marker = MARKER[line.kind]
        const numberForLine =
          line.kind === 'remove'
            ? padLeft(String(oldLine++), gutter - 1)
            : line.kind === 'add'
              ? padLeft(String(newLine++), gutter - 1)
              : (() => {
                  const n = padLeft(String(oldLine), gutter - 1)
                  oldLine++
                  newLine++
                  return n
                })()
        return (
          <text key={index} fg={COLOR[line.kind]} selectable>
            {marker} {numberForLine} {line.text}
          </text>
        )
      })}
      {hiddenCount > 0 && (
        <text fg={c.dim}>{`\u2026 +${hiddenCount} more lines`}</text>
      )}
    </box>
  )
}

import React from 'react'
import { c } from '../../theme.js'
import { hunkGutterWidth, type DiffHunk, type DiffLine } from './hunks.js'

/**
 * Render a list of unified-diff hunks with theme colours and a
 * line-number gutter. Lite-native replacement for the sample tree's
 * `StructuredDiff` + `StructuredDiffList`
 * (`ui/examples/upstream-patterns/src/components/StructuredDiff.tsx`,
 * `ui/examples/upstream-patterns/src/components/StructuredDiffList.tsx`)
 * that avoids the Rust NAPI colouriser and the `diff` runtime.
 *
 * Renders each hunk with a header row (`@@ -a,b +c,d @@`) followed by
 * marker-tagged lines. Context lines are dimmed; adds are green;
 * removes are red.
 */

type Props = {
  hunks: DiffHunk[]
  /** When set, show at most this many lines per hunk (context + edits
   * combined) so long hunks don't overflow the bubble. */
  maxLinesPerHunk?: number
  /** When true, swallow the hunk header row. Used by callers that draw
   * their own frame (e.g. the permission preview). */
  hideHeader?: boolean
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

function Hunk({
  hunk,
  maxLinesPerHunk,
  hideHeader,
}: {
  hunk: DiffHunk
  maxLinesPerHunk?: number
  hideHeader?: boolean
}) {
  const gutter = hunkGutterWidth(hunk)
  let oldLine = hunk.oldStart
  let newLine = hunk.newStart
  const truncated =
    typeof maxLinesPerHunk === 'number' && hunk.lines.length > maxLinesPerHunk
  const visible = truncated ? hunk.lines.slice(0, maxLinesPerHunk) : hunk.lines
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
        <text fg={c.dim}>\u2026 +{hiddenCount} more lines</text>
      )}
    </box>
  )
}

export function StructuredDiff({ hunks, maxLinesPerHunk, hideHeader }: Props) {
  if (hunks.length === 0) return null
  return (
    <box flexDirection="column">
      {hunks.map((hunk, index) => (
        <Hunk
          key={index}
          hunk={hunk}
          maxLinesPerHunk={maxLinesPerHunk}
          hideHeader={hideHeader}
        />
      ))}
    </box>
  )
}

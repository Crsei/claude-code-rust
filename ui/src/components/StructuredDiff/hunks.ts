/**
 * Minimal unified-diff parser and `old→new` hunk builder for the Lite
 * frontend. The sample tree's `StructuredDiff`
 * (`ui/examples/upstream-patterns/src/components/StructuredDiff.tsx`)
 * depends on the `diff` package plus a Rust NAPI colourizer. We recreate
 * just the shape the permission preview and file-edit result renderer
 * need — a list of hunks where each hunk is a list of marker-tagged
 * lines — in pure TypeScript.
 *
 * Hunk parser: parses a unified diff (`--- a\n+++ b\n@@ -1,3 +1,3 @@\n…`)
 * into `DiffHunk[]`. Unknown lines are ignored so fenced-diff tool
 * output and plain diffs both go through the same parser.
 *
 * Simple hunk builder: given `old_string` and `new_string`, produces a
 * single `DiffHunk` that marks every old line `-`, every new line `+`.
 * Good enough for file-edit preview before running LCS.
 */

export type DiffLineKind = 'context' | 'add' | 'remove'

export interface DiffLine {
  kind: DiffLineKind
  /** Line content without the leading `+`, `-`, or ` ` marker. */
  text: string
}

export interface DiffHunk {
  /** 1-based start line of the old file section. 0 if unknown. */
  oldStart: number
  /** Number of old lines the hunk covers. */
  oldLines: number
  /** 1-based start line of the new file section. 0 if unknown. */
  newStart: number
  /** Number of new lines the hunk covers. */
  newLines: number
  lines: DiffLine[]
}

const HUNK_HEADER = /^@@\s+-(\d+)(?:,(\d+))?\s+\+(\d+)(?:,(\d+))?\s+@@/

/**
 * Parse a unified diff into hunks. Lines outside any hunk header (e.g.
 * `--- foo`, `+++ foo`, `diff --git …`) are dropped. Returns an empty
 * array when no hunk header is found.
 */
export function parseUnifiedDiff(text: string): DiffHunk[] {
  if (!text) return []

  const hunks: DiffHunk[] = []
  let current: DiffHunk | null = null

  const raw = text.split(/\r?\n/)
  for (const line of raw) {
    const header = HUNK_HEADER.exec(line)
    if (header) {
      current = {
        oldStart: parseInt(header[1]!, 10),
        oldLines: header[2] ? parseInt(header[2], 10) : 1,
        newStart: parseInt(header[3]!, 10),
        newLines: header[4] ? parseInt(header[4], 10) : 1,
        lines: [],
      }
      hunks.push(current)
      continue
    }

    if (!current) continue

    if (line.startsWith('+++') || line.startsWith('---')) {
      // File-path headers sometimes appear interleaved — ignore them.
      continue
    }
    if (line.startsWith('+')) {
      current.lines.push({ kind: 'add', text: line.slice(1) })
    } else if (line.startsWith('-')) {
      current.lines.push({ kind: 'remove', text: line.slice(1) })
    } else if (line.startsWith(' ')) {
      current.lines.push({ kind: 'context', text: line.slice(1) })
    } else if (line === '\\ No newline at end of file') {
      // Skip the trailing marker but keep the hunk.
      continue
    } else if (line.length === 0) {
      // Blank line inside a hunk is treated as a context line.
      current.lines.push({ kind: 'context', text: '' })
    }
  }

  return hunks
}

/**
 * Build a single `DiffHunk` from `old_string` / `new_string` inputs
 * (as seen in `Edit` / `MultiEdit` / `Write` tool calls). Produces a
 * hunk where every old line is marked `-` and every new line is marked
 * `+`, with no context lines. Good enough to preview what a proposed
 * edit will change.
 */
export function hunkFromEdit(
  oldString: string,
  newString: string,
  options: { oldStart?: number; newStart?: number } = {},
): DiffHunk {
  const oldLinesArr = oldString === '' ? [] : oldString.split('\n')
  const newLinesArr = newString === '' ? [] : newString.split('\n')
  const lines: DiffLine[] = [
    ...oldLinesArr.map<DiffLine>(text => ({ kind: 'remove', text })),
    ...newLinesArr.map<DiffLine>(text => ({ kind: 'add', text })),
  ]
  return {
    oldStart: options.oldStart ?? 1,
    oldLines: oldLinesArr.length,
    newStart: options.newStart ?? 1,
    newLines: newLinesArr.length,
    lines,
  }
}

/**
 * Gutter width (digits in the largest line number in the hunk, plus
 * two: one for the marker column and one for the trailing space). Used
 * to pad line-number columns consistently across lines within a hunk.
 */
export function hunkGutterWidth(hunk: DiffHunk): number {
  const maxLine = Math.max(
    hunk.oldStart + hunk.oldLines - 1,
    hunk.newStart + hunk.newLines - 1,
    1,
  )
  return String(maxLine).length + 2
}

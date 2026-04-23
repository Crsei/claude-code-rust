import React from 'react'
import { c } from '../theme.js'
import { FilePathLink } from './FilePathLink.js'
import {
  StructuredDiffList,
  type DiffHunk,
  type DiffLine,
} from './StructuredDiff/index.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/FileEditToolUpdatedMessage.tsx`.
 *
 * Shown in the tool-result stream after a successful `Edit` / `MultiEdit`
 * / `Write`. Upstream tallies adds/removes across the `StructuredPatchHunk[]`
 * the backend ships back and prints `Added 3 lines, removed 2 lines` above
 * the diff. cc-rust ships the same summary: we recompute the counts from
 * the hunk list so callers can feed us either a backend-supplied
 * `DiffHunk[]` (e.g. from a future structured `tool_result` field) or the
 * result of `hunkFromEdit` off the tool input.
 *
 * The `previewHint` / condensed-mode rules match upstream:
 * - Plan files (`previewHint` set) collapse to a dim hint in normal mode
 *   and expand to the diff in condensed mode (subagent view).
 * - Condensed mode without `previewHint` just shows the add/remove
 *   one-liner — used when the parent already draws a diff adjacent.
 */

type Props = {
  filePath: string
  hunks: DiffHunk[]
  /** First line of the target file, when the caller has one — shown
   *  above the diff so long paths retain context. */
  firstLine?: string | null
  style?: 'condensed'
  verbose: boolean
  /** Hint for plan-file previews ("type /plan to see full content"). */
  previewHint?: string
  /** Hunk-level truncation cap. */
  maxLinesPerHunk?: number
}

function countKind(hunks: DiffHunk[], kind: DiffLine['kind']): number {
  let n = 0
  for (const hunk of hunks) {
    for (const line of hunk.lines) {
      if (line.kind === kind) n++
    }
  }
  return n
}

function Summary({ adds, removes }: { adds: number; removes: number }) {
  const parts: React.ReactNode[] = []
  if (adds > 0) {
    parts.push(
      <text key="add">
        Added <strong>{adds}</strong> {adds === 1 ? 'line' : 'lines'}
      </text>,
    )
  }
  if (adds > 0 && removes > 0) {
    parts.push(<text key="sep">, </text>)
  }
  if (removes > 0) {
    parts.push(
      <text key="rem">
        {adds === 0 ? 'R' : 'r'}emoved <strong>{removes}</strong>{' '}
        {removes === 1 ? 'line' : 'lines'}
      </text>,
    )
  }
  return <box flexDirection="row">{parts}</box>
}

export function FileEditToolUpdatedMessage({
  filePath,
  hunks,
  firstLine,
  style,
  verbose,
  previewHint,
  maxLinesPerHunk = 8,
}: Props) {
  const adds = countKind(hunks, 'add')
  const removes = countKind(hunks, 'remove')

  // Plan-file rules — invert condensed behaviour.
  if (previewHint) {
    if (style !== 'condensed' && !verbose) {
      return (
        <box paddingX={1} width="100%">
          <text fg={c.dim} selectable>
            {previewHint}
          </text>
        </box>
      )
    }
  } else if (style === 'condensed' && !verbose) {
    return (
      <box paddingX={1} width="100%">
        <Summary adds={adds} removes={removes} />
      </box>
    )
  }

  return (
    <box flexDirection="column" paddingX={1} width="100%">
      <box flexDirection="row" gap={1}>
        <FilePathLink filePath={filePath} />
        <Summary adds={adds} removes={removes} />
      </box>
      {firstLine && (
        <text fg={c.dim} selectable>
          {firstLine}
        </text>
      )}
      <box marginTop={1} paddingLeft={1}>
        <StructuredDiffList
          hunks={hunks}
          maxLinesPerHunk={verbose ? undefined : maxLinesPerHunk}
        />
      </box>
    </box>
  )
}

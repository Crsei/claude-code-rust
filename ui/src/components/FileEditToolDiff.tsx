import React from 'react'
import type { FileEditContext } from '../adapters/index.js'
import { extractFileEditContext } from '../adapters/index.js'
import { c } from '../theme.js'
import { FilePathLink } from './FilePathLink.js'
import {
  StructuredDiffList,
  hunkFromEdit,
  type DiffHunk,
} from './StructuredDiff/index.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/FileEditToolDiff.tsx`.
 *
 * Upstream snapshots the file on mount, reads a context window around
 * the edited range, runs `structuredPatch` to compute a unified diff,
 * and hands the result to `StructuredDiffList` for rendering. That path
 * depends on the Node `fs` runtime plus the `diff` package — both
 * unavailable to the cc-rust frontend, which has no direct filesystem
 * access.
 *
 * Instead we build the preview from the tool inputs themselves using
 * `hunkFromEdit`: every `old_string` line is marked `-`, every
 * `new_string` line `+`. This is the same primitive the permission
 * dialog's diff preview uses (`messages/FileEditToolPreview`), so the
 * two renderers stay visually consistent. Callers that already have a
 * pre-computed hunk list (e.g. the backend forwards a
 * `StructuredPatchHunk[]`) can skip the input coercion by passing
 * `hunks` directly.
 */

type Props = {
  /** Absolute path of the edited file. */
  file_path: string
  /**
   * Raw tool input. Accepts the `{file_path, old_string, new_string}`
   * shape the `Edit` tool ships as well as `{file_path, edits: [...]}`
   * from `MultiEdit`. Ignored when `hunks` is passed.
   */
  edits?: unknown
  /** Pre-computed hunks — skip the input coercion path. */
  hunks?: DiffHunk[]
  /** Collapse very long hunks. Matches the default cap used by the
   *  inline preview in `messages/FileEditToolPreview`. */
  maxLinesPerHunk?: number
}

const DEFAULT_MAX_LINES = 8

function hunksFromInput(
  file_path: string,
  edits: unknown,
): { context: FileEditContext | null; hunks: DiffHunk[] } {
  const context = extractFileEditContext('Edit', { file_path, edits })
  if (!context) return { context: null, hunks: [] }
  const hunks = context.edits.map(edit =>
    hunkFromEdit(edit.oldString, edit.newString),
  )
  return { context, hunks }
}

export function FileEditToolDiff({
  file_path,
  edits,
  hunks: preComputed,
  maxLinesPerHunk = DEFAULT_MAX_LINES,
}: Props) {
  const { context, hunks } = preComputed
    ? { context: null, hunks: preComputed }
    : hunksFromInput(file_path, edits)

  if (hunks.length === 0) {
    return null
  }

  const editCount = context?.edits.length ?? hunks.length
  const editLabel = editCount === 1 ? '1 edit' : `${editCount} edits`

  return (
    <box flexDirection="column" marginTop={1} width="100%">
      <box flexDirection="row" gap={1}>
        <text fg={c.dim}>File</text>
        <FilePathLink filePath={file_path} />
        <text fg={c.dim}>({editLabel})</text>
      </box>
      <box
        marginTop={1}
        paddingLeft={1}
        flexDirection="column"
        border={['top', 'bottom']}
        borderColor={c.dim}
      >
        <StructuredDiffList
          hunks={hunks}
          maxLinesPerHunk={maxLinesPerHunk}
          hideHeader
        />
      </box>
    </box>
  )
}

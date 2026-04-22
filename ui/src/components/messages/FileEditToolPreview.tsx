import React from 'react'
import { extractFileEditContext } from '../../adapters/index.js'
import { c } from '../../theme.js'
import { FilePathLink } from '../FilePathLink.js'
import {
  StructuredDiff,
  hunkFromEdit,
  type DiffHunk,
} from '../StructuredDiff/index.js'

/**
 * Mini-diff rendering used inside a `tool_activity` row when the tool
 * is a file-edit / file-write. Lite-native counterpart of the sample
 * tree's `FileEditToolUpdatedMessage`
 * (`ui/examples/upstream-patterns/src/components/FileEditToolUpdatedMessage.tsx`)
 * and `FileEditToolDiff`, collapsed to the always-on "show what
 * changed" slice.
 *
 * Returns `null` when the tool input doesn't describe a file edit —
 * callers treat a null response as "nothing to preview, keep the
 * generic tool rendering intact".
 */

type Props = {
  toolName: string
  input: unknown
  /** Maximum lines to render per hunk before collapsing. */
  maxLinesPerHunk?: number
}

const DEFAULT_MAX_LINES = 8

export function FileEditToolPreview({
  toolName,
  input,
  maxLinesPerHunk = DEFAULT_MAX_LINES,
}: Props) {
  const context = extractFileEditContext(toolName, input)
  if (!context) return null

  const hunks: DiffHunk[] = context.edits.map(edit =>
    hunkFromEdit(edit.oldString, edit.newString),
  )

  const editLabel =
    context.edits.length === 1
      ? '1 edit'
      : `${context.edits.length} edits`

  return (
    <box flexDirection="column" marginTop={1}>
      <box flexDirection="row" gap={1}>
        <text fg={c.dim}>File</text>
        <FilePathLink filePath={context.filePath} />
        <text fg={c.dim}>({editLabel})</text>
      </box>
      <box marginTop={1} paddingLeft={1} flexDirection="column">
        <StructuredDiff hunks={hunks} maxLinesPerHunk={maxLinesPerHunk} hideHeader />
      </box>
    </box>
  )
}

/** Tool names that should surface the inline edit preview. */
const FILE_EDIT_TOOLS = new Set([
  'Edit',
  'MultiEdit',
  'FileEdit',
  'NotebookEdit',
  'Write',
  'FileWrite',
])

export function isFileEditToolName(name: string): boolean {
  return FILE_EDIT_TOOLS.has(name)
}

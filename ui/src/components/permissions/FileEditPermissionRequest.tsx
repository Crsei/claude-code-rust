import React from 'react'
import { c } from '../../theme.js'
import { FilePathLink } from '../FilePathLink.js'
import { StructuredDiff, hunkFromEdit, type DiffHunk } from '../StructuredDiff/index.js'
import type { FileEditContext } from '../../adapters/index.js'

/**
 * Body for the permission dialog when the requested tool wants to edit
 * an existing file. Ports the spirit of the sample tree's
 * `FileEditPermissionRequest`
 * (`ui/examples/upstream-patterns/src/components/permissions/FileEditPermissionRequest/`)
 * but runs on the Lite structured-diff renderer — no file-read
 * pre-flight, no NAPI, no suspense.
 *
 * If the tool input doesn't parse into a `FileEditContext` we show the
 * raw command as a fallback.
 */

type Props = {
  context: FileEditContext | null
  fallbackCommand: string
}

const MAX_LINES_PER_HUNK = 12

function contextToHunks(context: FileEditContext): DiffHunk[] {
  return context.edits.map(edit => hunkFromEdit(edit.oldString, edit.newString))
}

export function FileEditPermissionRequest({ context, fallbackCommand }: Props) {
  if (!context) {
    return (
      <box flexDirection="column">
        <text fg={c.dim}>Proposed change</text>
        <box border={['left']} borderColor={c.warning} paddingLeft={1} paddingRight={1}>
          <text selectable>{fallbackCommand}</text>
        </box>
      </box>
    )
  }

  const hunks = contextToHunks(context)
  const editLabel =
    context.edits.length === 1
      ? '1 edit'
      : `${context.edits.length} edits`

  return (
    <box flexDirection="column">
      <box flexDirection="row" gap={1}>
        <text fg={c.dim}>File</text>
        <FilePathLink filePath={context.filePath} />
        <text fg={c.dim}>({editLabel})</text>
      </box>
      <box marginTop={1} border={['left']} borderColor={c.warning} paddingLeft={1} paddingRight={1} flexDirection="column">
        <StructuredDiff hunks={hunks} maxLinesPerHunk={MAX_LINES_PER_HUNK} hideHeader />
      </box>
    </box>
  )
}

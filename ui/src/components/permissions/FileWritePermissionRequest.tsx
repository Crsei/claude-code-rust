import React from 'react'
import { c } from '../../theme.js'
import { FilePathLink } from '../FilePathLink.js'
import type { FileEditContext } from '../../adapters/index.js'

/**
 * Body for the permission dialog when the requested tool wants to
 * write a (potentially new) file. Shows the file path as a clickable
 * link and a capped preview of the first lines of the content.
 * Mirrors the sample tree's `FileWritePermissionRequest`
 * (`ui/examples/upstream-patterns/src/components/permissions/FileWritePermissionRequest/`).
 */

type Props = {
  context: FileEditContext | null
  fallbackCommand: string
}

const MAX_PREVIEW_LINES = 10
const MAX_CHARS_PER_LINE = 120

function capLine(line: string): string {
  if (line.length <= MAX_CHARS_PER_LINE) return line
  return `${line.slice(0, MAX_CHARS_PER_LINE - 1)}\u2026`
}

export function FileWritePermissionRequest({ context, fallbackCommand }: Props) {
  if (!context || context.edits.length === 0) {
    return (
      <box flexDirection="column">
        <text fg={c.dim}>Proposed write</text>
        <box border={['left']} borderColor={c.warning} paddingLeft={1} paddingRight={1}>
          <text selectable>{fallbackCommand}</text>
        </box>
      </box>
    )
  }

  const entry = context.edits[0]!
  const allLines = entry.newString.split('\n')
  const previewLines = allLines.slice(0, MAX_PREVIEW_LINES)
  const hiddenLines = allLines.length - previewLines.length

  return (
    <box flexDirection="column">
      <box flexDirection="row" gap={1}>
        <text fg={c.dim}>File</text>
        <FilePathLink filePath={context.filePath} />
        <text fg={c.dim}>
          ({allLines.length} line{allLines.length === 1 ? '' : 's'})
        </text>
      </box>
      <box
        marginTop={1}
        border={['left']}
        borderColor={c.warning}
        paddingLeft={1}
        paddingRight={1}
        flexDirection="column"
      >
        {previewLines.map((line, i) => (
          <text key={i} fg={c.success} selectable>
            + {capLine(line)}
          </text>
        ))}
        {hiddenLines > 0 && (
          <text fg={c.dim}>
            \u2026 +{hiddenLines} more line{hiddenLines === 1 ? '' : 's'}
          </text>
        )}
      </box>
    </box>
  )
}

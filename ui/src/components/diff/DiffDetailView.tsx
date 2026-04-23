import React from 'react'
import { c } from '../../theme.js'
import {
  StructuredDiff,
  type DiffHunk,
} from '../StructuredDiff/index.js'
import { Divider } from '../design-system/Divider.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/diff/DiffDetailView.tsx`.
 *
 * Upstream reads the file from disk for syntax-detection context; the
 * Lite frontend doesn't run in a full Node environment, so the component
 * expects the caller to pre-parse its hunks into the Lite `DiffHunk`
 * shape (see `components/StructuredDiff/hunks.ts`) and pass them in
 * directly.
 */

type Props = {
  filePath: string
  hunks: DiffHunk[]
  isLargeFile?: boolean
  isBinary?: boolean
  isTruncated?: boolean
  isUntracked?: boolean
  /** Cap rendered lines per hunk. Passed through to `<StructuredDiff>`. */
  maxLinesPerHunk?: number
}

export function DiffDetailView({
  filePath,
  hunks,
  isLargeFile = false,
  isBinary = false,
  isTruncated = false,
  isUntracked = false,
  maxLinesPerHunk,
}: Props) {
  if (isUntracked) {
    return (
      <box flexDirection="column" width="100%">
        <box>
          <strong><text>{filePath}</text></strong>
          <text fg={c.dim}> (untracked)</text>
        </box>
        <Divider padding={4} />
        <box flexDirection="column">
          <em><text fg={c.dim}>New file not yet staged.</text></em>
          <em><text fg={c.dim}>Run `git add {filePath}` to see line counts.</text></em>
        </box>
      </box>
    )
  }

  if (isBinary) {
    return (
      <box flexDirection="column" width="100%">
        <strong><text>{filePath}</text></strong>
        <Divider padding={4} />
        <em><text fg={c.dim}>Binary file — cannot display diff</text></em>
      </box>
    )
  }

  if (isLargeFile) {
    return (
      <box flexDirection="column" width="100%">
        <strong><text>{filePath}</text></strong>
        <Divider padding={4} />
        <em><text fg={c.dim}>Large file — diff exceeds 1 MB limit</text></em>
      </box>
    )
  }

  return (
    <box flexDirection="column" width="100%">
      <box flexDirection="row">
        <strong><text>{filePath}</text></strong>
        {isTruncated && <text fg={c.dim}> (truncated)</text>}
      </box>

      <Divider padding={4} />

      <box flexDirection="column">
        {hunks.length === 0 ? (
          <text fg={c.dim}>No diff content</text>
        ) : (
          <StructuredDiff hunks={hunks} maxLinesPerHunk={maxLinesPerHunk} />
        )}
      </box>

      {isTruncated && (
        <em>
          <text fg={c.dim}>… diff truncated (exceeded 400 line limit)</text>
        </em>
      )}
    </box>
  )
}

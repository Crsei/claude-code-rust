import React from 'react'
import { c } from '../theme.js'
import type { DiffHunk } from './StructuredDiff/hunks.js'
import { StructuredDiff } from './StructuredDiff.js'

/**
 * Render a list of diff hunks with ellipsis separators between them.
 * Port of `ui/examples/upstream-patterns/src/components/StructuredDiffList.tsx`
 * to OpenTUI.
 */

type Props = {
  hunks: DiffHunk[]
  /** Passed through to per-hunk rendering. */
  maxLinesPerHunk?: number
  /** Passed through to per-hunk rendering. */
  hideHeader?: boolean
  /** Accepted for API compatibility. */
  dim?: boolean
  width?: number
  filePath?: string
  firstLine?: string | null
  fileContent?: string
}

export function StructuredDiffList({
  hunks,
  maxLinesPerHunk,
  hideHeader,
  dim,
  width,
  filePath,
  firstLine,
  fileContent,
}: Props): React.ReactNode {
  if (hunks.length === 0) return null

  const children: React.ReactNode[] = []
  hunks.forEach((hunk, index) => {
    if (index > 0) {
      children.push(
        <text key={`ellipsis-${index}`} fg={c.dim}>
          ...
        </text>,
      )
    }
    children.push(
      <box key={`hunk-${hunk.newStart}-${index}`} flexDirection="column">
        <StructuredDiff
          hunk={hunk}
          maxLinesPerHunk={maxLinesPerHunk}
          hideHeader={hideHeader}
          dim={dim}
          width={width}
          filePath={filePath}
          firstLine={firstLine}
          fileContent={fileContent}
        />
      </box>,
    )
  })

  return <box flexDirection="column">{children}</box>
}

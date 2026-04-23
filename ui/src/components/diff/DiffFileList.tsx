import React, { useMemo } from 'react'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/diff/DiffFileList.tsx`.
 *
 * Renders a scrollable list of files with their `+added / -removed`
 * stats next to each row. A simple fixed-size scroll window (5 items by
 * default) keeps the focused file roughly centred and shows "N more"
 * hints above / below when the list overflows.
 */

export type DiffFile = {
  path: string
  linesAdded: number
  linesRemoved: number
  isBinary?: boolean
  isLargeFile?: boolean
  isTruncated?: boolean
  isUntracked?: boolean
  isNewFile?: boolean
}

type Props = {
  files: DiffFile[]
  selectedIndex: number
  /** Max rows rendered before paginating (default 5, matches upstream). */
  maxVisible?: number
}

function plural(n: number, word: string): string {
  return n === 1 ? word : `${word}s`
}

function truncateStart(path: string, width: number): string {
  if (path.length <= width) return path
  return '\u2026' + path.slice(path.length - (width - 1))
}

export function DiffFileList({ files, selectedIndex, maxVisible = 5 }: Props) {
  const { startIndex, endIndex } = useMemo(() => {
    if (files.length === 0 || files.length <= maxVisible) {
      return { startIndex: 0, endIndex: files.length }
    }
    let start = Math.max(0, selectedIndex - Math.floor(maxVisible / 2))
    let end = start + maxVisible
    if (end > files.length) {
      end = files.length
      start = Math.max(0, end - maxVisible)
    }
    return { startIndex: start, endIndex: end }
  }, [files.length, selectedIndex, maxVisible])

  if (files.length === 0) {
    return <text fg={c.dim}>No changed files</text>
  }

  const visible = files.slice(startIndex, endIndex)
  const hasMoreAbove = startIndex > 0
  const hasMoreBelow = endIndex < files.length
  const needsPagination = files.length > maxVisible

  return (
    <box flexDirection="column">
      {needsPagination && (
        <text fg={c.dim}>
          {hasMoreAbove
            ? ` \u2191 ${startIndex} more ${plural(startIndex, 'file')}`
            : ' '}
        </text>
      )}
      {visible.map((file, i) => {
        const absIndex = startIndex + i
        const isSelected = absIndex === selectedIndex
        return (
          <FileItem key={file.path} file={file} isSelected={isSelected} />
        )
      })}
      {needsPagination && (
        <text fg={c.dim}>
          {hasMoreBelow
            ? ` \u2193 ${files.length - endIndex} more ${plural(files.length - endIndex, 'file')}`
            : ' '}
        </text>
      )}
    </box>
  )
}

function FileItem({ file, isSelected }: { file: DiffFile; isSelected: boolean }) {
  const pointer = isSelected ? '\u276F ' : '  '
  const displayPath = truncateStart(file.path, 60)

  return (
    <box flexDirection="row" justifyContent="space-between">
      <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
        {pointer}
        {displayPath}
      </text>
      <FileStats file={file} isSelected={isSelected} />
    </box>
  )
}

function FileStats({ file, isSelected }: { file: DiffFile; isSelected: boolean }) {
  if (file.isUntracked) {
    return (
      <em>
        <text fg={isSelected ? undefined : c.dim}>untracked</text>
      </em>
    )
  }
  if (file.isBinary) {
    return (
      <em>
        <text fg={isSelected ? undefined : c.dim}>Binary file</text>
      </em>
    )
  }
  if (file.isLargeFile) {
    return (
      <em>
        <text fg={isSelected ? undefined : c.dim}>Large file modified</text>
      </em>
    )
  }
  return (
    <text>
      {file.linesAdded > 0 && <text fg={c.success}>+{file.linesAdded}</text>}
      {file.linesAdded > 0 && file.linesRemoved > 0 && ' '}
      {file.linesRemoved > 0 && <text fg={c.error}>-{file.linesRemoved}</text>}
      {file.isTruncated && <text fg={c.dim}> (truncated)</text>}
    </text>
  )
}

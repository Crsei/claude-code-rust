import React, { useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'
import { Byline } from '../design-system/Byline.js'
import { Dialog } from '../design-system/Dialog.js'
import { KeyboardShortcutHint } from '../design-system/KeyboardShortcutHint.js'
import type { DiffHunk } from '../StructuredDiff/index.js'
import { DiffDetailView } from './DiffDetailView.js'
import { DiffFileList, type DiffFile } from './DiffFileList.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/diff/DiffDialog.tsx`.
 *
 * Upstream sources diff data from `useDiffData()` + `useTurnDiffs()`,
 * both of which touch Ink hooks and the on-disk git state. The Lite
 * port takes a pre-built `sources` array so the caller decides where
 * the data comes from (git, a turn snapshot, a fixture, …).
 *
 * Keyboard contract (from upstream):
 *  - ↑/↓   navigate files in list mode
 *  - ←/→   switch sources (multi-source mode) or go back from detail
 *  - Enter enter detail mode
 *  - Esc   close (or exit detail mode first)
 */

export type DiffStats = {
  filesCount: number
  linesAdded: number
  linesRemoved: number
}

export type DiffSourceData = {
  /** Display name for the source pill, e.g. "Current" or "T3". */
  label: string
  /** Shown next to the title: `"T3 \"initial scaffolding\""`. */
  title?: string
  subtitle?: string
  files: DiffFile[]
  hunks: Map<string, DiffHunk[]>
  stats?: DiffStats
  /** When true, the dialog renders a spinner instead of the list. */
  loading?: boolean
}

type Props = {
  sources: DiffSourceData[]
  onDone: (message?: string) => void
  /** Default source index, used when resuming a dialog. */
  initialSourceIndex?: number
  /** Default file index inside the initial source. */
  initialSelectedIndex?: number
}

type ViewMode = 'list' | 'detail'

function pluralFile(n: number): string {
  return n === 1 ? 'file' : 'files'
}

export function DiffDialog({
  sources,
  onDone,
  initialSourceIndex = 0,
  initialSelectedIndex = 0,
}: Props) {
  const [viewMode, setViewMode] = useState<ViewMode>('list')
  const [sourceIndex, setSourceIndex] = useState(
    Math.max(0, Math.min(initialSourceIndex, sources.length - 1)),
  )
  const [selectedIndex, setSelectedIndex] = useState(initialSelectedIndex)

  const currentSource = sources[sourceIndex]
  const files = currentSource?.files ?? []
  const selectedFile = files[selectedIndex]
  const selectedHunks = useMemo(() => {
    if (!selectedFile) return [] as DiffHunk[]
    return currentSource?.hunks.get(selectedFile.path) ?? []
  }, [currentSource, selectedFile])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name

    if (name === 'escape') {
      if (viewMode === 'detail') setViewMode('list')
      else onDone('Diff dialog dismissed')
      return
    }
    if (viewMode === 'detail') {
      if (name === 'left') setViewMode('list')
      return
    }
    if (name === 'left') {
      if (sources.length > 1) {
        setSourceIndex(prev => {
          const next = Math.max(0, prev - 1)
          if (next !== prev) setSelectedIndex(0)
          return next
        })
      }
      return
    }
    if (name === 'right') {
      if (sources.length > 1) {
        setSourceIndex(prev => {
          const next = Math.min(sources.length - 1, prev + 1)
          if (next !== prev) setSelectedIndex(0)
          return next
        })
      }
      return
    }
    if (name === 'up') {
      setSelectedIndex(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down') {
      setSelectedIndex(prev => Math.min(files.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      if (selectedFile) setViewMode('detail')
    }
  })

  const stats = currentSource?.stats
  const subtitle = stats ? (
    <text fg={c.dim}>
      {stats.filesCount} {pluralFile(stats.filesCount)} changed
      {stats.linesAdded > 0 && <text fg={c.success}> +{stats.linesAdded}</text>}
      {stats.linesRemoved > 0 && <text fg={c.error}> -{stats.linesRemoved}</text>}
    </text>
  ) : null

  const sourceSelector =
    sources.length > 1 ? (
      <box flexDirection="row">
        {sourceIndex > 0 && <text fg={c.dim}>{'\u25C0 '}</text>}
        {sources.map((source, i) => {
          const isSelected = i === sourceIndex
          return (
            <text key={source.label}>
              {i > 0 ? <text fg={c.dim}> \u00B7 </text> : null}
              {isSelected ? (
                <strong><text>{source.label}</text></strong>
              ) : (
                <text fg={c.dim}>{source.label}</text>
              )}
            </text>
          )
        })}
        {sourceIndex < sources.length - 1 && <text fg={c.dim}>{' \u25B6'}</text>}
      </box>
    ) : null

  const headerTitle = currentSource?.title ?? 'Uncommitted changes'
  const headerSubtitle = currentSource?.subtitle ?? '(git diff HEAD)'

  const emptyMessage = currentSource?.loading
    ? 'Loading diff…'
    : currentSource && files.length === 0
      ? stats && stats.filesCount > 0
        ? 'Too many files to display details'
        : 'Working tree is clean'
      : 'No diff data'

  const guide =
    viewMode === 'list' ? (
      <Byline>
        {sources.length > 1 && <text>{'\u2190/\u2192'} source</text>}
        <text>{'\u2191/\u2193'} select</text>
        <KeyboardShortcutHint shortcut="Enter" action="view" />
        <KeyboardShortcutHint shortcut="Esc" action="close" />
      </Byline>
    ) : (
      <Byline>
        <text>{'\u2190'} back</text>
        <KeyboardShortcutHint shortcut="Esc" action="close" />
      </Byline>
    )

  return (
    <Dialog
      title={
        <text>
          {headerTitle}
          {headerSubtitle && <text fg={c.dim}>{' '}{headerSubtitle}</text>}
        </text>
      }
      onCancel={() => onDone('Diff dialog dismissed')}
      inputGuide={guide}
      color="background"
    >
      {sourceSelector}
      {subtitle}
      {files.length === 0 ? (
        <box marginTop={1}>
          <text fg={c.dim}>{emptyMessage}</text>
        </box>
      ) : viewMode === 'list' ? (
        <box flexDirection="column" marginTop={1}>
          <DiffFileList files={files} selectedIndex={selectedIndex} />
        </box>
      ) : (
        <box flexDirection="column" marginTop={1}>
          <DiffDetailView
            filePath={selectedFile?.path ?? ''}
            hunks={selectedHunks}
            isLargeFile={selectedFile?.isLargeFile}
            isBinary={selectedFile?.isBinary}
            isTruncated={selectedFile?.isTruncated}
            isUntracked={selectedFile?.isUntracked}
          />
        </box>
      )}
    </Dialog>
  )
}

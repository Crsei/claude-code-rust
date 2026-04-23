import React from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/NotebookEditToolUseRejectedMessage.tsx`.
 *
 * Rendered when the user rejects a NotebookEdit tool invocation. The
 * upstream uses `HighlightedCode` to show the (would-have-been-inserted)
 * cell source; OpenTUI's `<code>` primitive provides the equivalent
 * syntax-highlighted block.
 */

type EditMode = 'replace' | 'insert' | 'delete'
type CellType = 'code' | 'markdown'

type Props = {
  notebookPath: string
  cellId?: string
  newSource: string
  cellType?: CellType
  editMode?: EditMode
  /** When true, render the full path instead of a cwd-relative form. */
  verbose?: boolean
  /** Optional cwd used to relativise `notebookPath` when `!verbose`. */
  cwd?: string
}

function relativePath(from: string | undefined, to: string): string {
  if (!from) return to
  if (to.startsWith(from)) {
    const rest = to.slice(from.length)
    return rest.replace(/^[\\/]+/, '') || to
  }
  return to
}

function cellLanguage(cellType: CellType | undefined): string {
  return cellType === 'markdown' ? 'markdown' : 'python'
}

export function NotebookEditToolUseRejectedMessage({
  notebookPath,
  cellId,
  newSource,
  cellType,
  editMode = 'replace',
  verbose = false,
  cwd,
}: Props) {
  const operation = editMode === 'delete' ? 'delete' : `${editMode} cell in`
  const displayPath = verbose ? notebookPath : relativePath(cwd, notebookPath)

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1}>
      <box flexDirection="row">
        <text fg={c.dim}>User rejected {operation} </text>
        <strong>
          <text fg={c.dim}>{displayPath}</text>
        </strong>
        {cellId && <text fg={c.dim}> at cell {cellId}</text>}
      </box>
      {editMode !== 'delete' && newSource.length > 0 && (
        <box marginTop={1} flexDirection="column">
          <code code={newSource} language={cellLanguage(cellType)} />
        </box>
      )}
    </box>
  )
}

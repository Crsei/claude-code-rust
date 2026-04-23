import React from 'react'
import { c } from '../theme.js'
import { FilePathLink } from './FilePathLink.js'
import { HighlightedCode } from './HighlightedCode.js'
import {
  StructuredDiff,
  type DiffHunk,
} from './StructuredDiff/index.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/FileEditToolUseRejectedMessage.tsx`.
 *
 * Rendered when the user denied a pending `Edit` / `MultiEdit` / `Write`.
 * The upstream component uses `path.relative(getCwd(), …)` to condense
 * long absolute paths; cc-rust's `FilePathLink` already handles display
 * with OSC 8, so we pass the raw path through and let the terminal
 * render a relative form itself via the OSC 8 label. `HighlightedCode`
 * renders the `Write` preview in dim style to convey the rejected state.
 */

const MAX_LINES_TO_RENDER = 10

type Props = {
  file_path: string
  operation: 'write' | 'update'
  /** For `update`: unified-diff hunks. */
  hunks?: DiffHunk[]
  /** For `write`: proposed full file contents. */
  content?: string
  /** First line of the target file; shown above the diff for context. */
  firstLine?: string | null
  /** Matches upstream's `style = 'condensed'` — collapses to a single-row
   *  header, used by subagent views. */
  style?: 'condensed'
  verbose: boolean
}

function Header({ file_path, operation }: { file_path: string; operation: Props['operation'] }) {
  return (
    <box flexDirection="row" gap={1}>
      <box minWidth={2} flexShrink={0}>
        <text fg={c.warning}>{'\u25A0'}</text>
      </box>
      <text fg={c.dim}>User rejected {operation} to</text>
      <FilePathLink filePath={file_path} fg={c.dim} />
    </box>
  )
}

export function FileEditToolUseRejectedMessage({
  file_path,
  operation,
  hunks,
  content,
  firstLine,
  style,
  verbose,
}: Props) {
  if (style === 'condensed' && !verbose) {
    return (
      <box paddingX={1} width="100%">
        <Header file_path={file_path} operation={operation} />
      </box>
    )
  }

  // New-file creation — render dimmed content preview, truncating in
  // non-verbose mode.
  if (operation === 'write' && content !== undefined) {
    const lines = content.split('\n')
    const plusLines = lines.length - MAX_LINES_TO_RENDER
    const truncated = verbose
      ? content
      : lines.slice(0, MAX_LINES_TO_RENDER).join('\n')

    return (
      <box flexDirection="column" paddingX={1} width="100%">
        <Header file_path={file_path} operation={operation} />
        <box marginTop={1} paddingLeft={3}>
          <HighlightedCode
            code={truncated || '(No content)'}
            filePath={file_path}
            dim
          />
        </box>
        {!verbose && plusLines > 0 && (
          <text fg={c.dim}>
            {'\u2026 +'}
            {plusLines} {plusLines === 1 ? 'line' : 'lines'}
          </text>
        )}
      </box>
    )
  }

  // Update — render diff, dimmed.
  if (!hunks || hunks.length === 0) {
    return (
      <box paddingX={1} width="100%">
        <Header file_path={file_path} operation={operation} />
      </box>
    )
  }

  return (
    <box flexDirection="column" paddingX={1} width="100%">
      <Header file_path={file_path} operation={operation} />
      {firstLine && (
        <text fg={c.dim} selectable>
          {firstLine}
        </text>
      )}
      <box marginTop={1} paddingLeft={3}>
        <StructuredDiff
          hunks={hunks}
          maxLinesPerHunk={verbose ? undefined : MAX_LINES_TO_RENDER}
        />
      </box>
    </box>
  )
}

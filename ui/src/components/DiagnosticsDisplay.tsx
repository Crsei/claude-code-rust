import React from 'react'
import { useAppState } from '../store/app-store.js'
import type { LspDiagnostic } from '../ipc/protocol.js'
import { c } from '../theme.js'
import { FilePathLink } from './FilePathLink.js'

/**
 * Native port of
 * `ui/examples/upstream-patterns/src/components/DiagnosticsDisplay.tsx`.
 *
 * Upstream reads an `Attachment` discriminant of type `diagnostics` that
 * the REPL inlines into the transcript. cc-rust has no attachment
 * pipeline; instead we wire directly to `state.diagnostics.byUri`,
 * populated from the `LspEvent::DiagnosticsPublished` IPC event (added
 * together with this component, see `store/app-state.ts` and
 * `store/reducers/subsystems.ts`).
 *
 * Two display modes — matching upstream:
 * - `verbose = false` (default): aggregate summary "Found N new
 *   diagnostic issue(s) in M file(s)". Suppresses the line entirely
 *   when there are no diagnostics.
 * - `verbose = true`: per-file breakdown with severity symbol, line /
 *   column, message, optional `code` / `source` annotations.
 *
 * The component accepts an optional `uriFilter` so callers can scope the
 * view to a single file (e.g. an inline panel above `FileEditTool`).
 */

type Props = {
  /** Forced verbose mode. Defaults to `false`. */
  verbose?: boolean
  /** When set, only render diagnostics for URIs that pass this filter. */
  uriFilter?: (uri: string) => boolean
  /** Alternate data source — skips the store read. Used by tests. */
  overrideByUri?: Record<string, LspDiagnostic[]>
}

interface FileGroup {
  uri: string
  display: string
  diagnostics: LspDiagnostic[]
}

function severitySymbol(severity: string): string {
  switch (severity.toLowerCase()) {
    case 'error':
      return '\u2717' // ✗
    case 'warning':
    case 'warn':
      return '\u26A0' // ⚠
    case 'information':
    case 'info':
      return '\u2139' // ℹ
    case 'hint':
      return '\u25E6' // ◦
    default:
      return '\u2022' // •
  }
}

function severityColor(severity: string): string {
  switch (severity.toLowerCase()) {
    case 'error':
      return c.error
    case 'warning':
    case 'warn':
      return c.warning
    default:
      return c.dim
  }
}

function toDisplayPath(uri: string): string {
  if (uri.startsWith('file://')) {
    // `file://host/absolute/path` → `/absolute/path`; drop platform-specific
    // prefixes Rust sends for temp buffers. Upstream uses
    // `path.relative(getCwd(), …)` — we keep the raw path since OSC 8
    // hyperlinks handle the display collapse.
    return decodeURI(uri.replace(/^file:\/\/(?:[^/]*)/, ''))
  }
  return uri
}

export function DiagnosticsDisplay({
  verbose = false,
  uriFilter,
  overrideByUri,
}: Props) {
  const diagnosticsState = useAppState().diagnostics
  const byUri = overrideByUri ?? diagnosticsState.byUri

  const groups: FileGroup[] = Object.entries(byUri)
    .filter(([uri, diagnostics]) => {
      if (diagnostics.length === 0) return false
      if (uriFilter && !uriFilter(uri)) return false
      return true
    })
    .map(([uri, diagnostics]) => ({
      uri,
      display: toDisplayPath(uri),
      diagnostics,
    }))

  if (groups.length === 0) return null

  const totalIssues = groups.reduce(
    (sum, group) => sum + group.diagnostics.length,
    0,
  )
  const fileCount = groups.length

  if (!verbose) {
    return (
      <box paddingX={1} width="100%">
        <text fg={c.dim} selectable>
          Found <strong>{totalIssues}</strong> new diagnostic{' '}
          {totalIssues === 1 ? 'issue' : 'issues'} in <strong>{fileCount}</strong>{' '}
          {fileCount === 1 ? 'file' : 'files'} (Ctrl+O to expand)
        </text>
      </box>
    )
  }

  return (
    <box flexDirection="column" paddingX={1} width="100%">
      {groups.map(group => (
        <box key={group.uri} flexDirection="column" marginBottom={1}>
          <box flexDirection="row" gap={1}>
            <FilePathLink filePath={group.display} fg={c.dim} />
            <text fg={c.dim}>
              ({group.diagnostics.length}{' '}
              {group.diagnostics.length === 1 ? 'issue' : 'issues'}):
            </text>
          </box>
          {group.diagnostics.map((diagnostic, index) => {
            const color = severityColor(diagnostic.severity)
            const line = diagnostic.range.start_line
            const column = diagnostic.range.start_character
            const tail = [
              diagnostic.code ? `[${diagnostic.code}]` : undefined,
              diagnostic.source ? `(${diagnostic.source})` : undefined,
            ]
              .filter(Boolean)
              .join(' ')
            return (
              <box key={index} flexDirection="row" paddingLeft={2}>
                <text fg={color}>{severitySymbol(diagnostic.severity)} </text>
                <text fg={c.dim} selectable>
                  [Line {line}:{column}]
                </text>
                <text fg={c.text} selectable>
                  {' '}
                  {diagnostic.message}
                  {tail ? ` ${tail}` : ''}
                </text>
              </box>
            )
          })}
        </box>
      ))}
    </box>
  )
}

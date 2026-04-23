import React, { useMemo } from 'react'
import { c } from '../../theme.js'
import { stripAnsi } from './format.js'
import { useExpandShellOutput } from './ExpandShellOutputContext.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/shell/OutputLine.tsx`.
 *
 * Differences from upstream:
 * - No Ink `<Ansi>`: OpenTUI `<text>` renders literal characters, so we
 *   strip ANSI with `stripAnsi` rather than colorising inline.
 * - No `MessageResponse` wrapper: the shell components that consume this
 *   own the `⎿` gutter. `OutputLine` is purely the body text.
 * - No upstream `renderTruncatedContent`: when truncation is required we
 *   keep the last `maxLines` lines as a tail — matches the behaviour of
 *   the upstream `ShellProgressMessage` tail-preview.
 */

const MAX_JSON_FORMAT_LENGTH = 10_000
const DEFAULT_MAX_LINES = 8

type Props = {
  content: string
  /** If true, render the full (stripped) content. */
  verbose: boolean
  isError?: boolean
  isWarning?: boolean
  /**
   * Max lines to show when `verbose` is false. Ignored while expanded via
   * `ExpandShellOutputContext`.
   */
  maxLines?: number
}

export function tryFormatJson(line: string): string {
  try {
    const parsed = JSON.parse(line)
    const stringified = JSON.stringify(parsed)

    // Precision loss guard — large 64-bit ints lose precision through
    // JSON.parse/stringify. Normalise both strings for comparison.
    const normalizedOriginal = line.replace(/\\\//g, '/').replace(/\s+/g, '')
    const normalizedStringified = stringified.replace(/\s+/g, '')

    if (normalizedOriginal !== normalizedStringified) {
      return line
    }

    return JSON.stringify(parsed, null, 2)
  } catch {
    return line
  }
}

export function tryJsonFormatContent(content: string): string {
  if (content.length > MAX_JSON_FORMAT_LENGTH) {
    return content
  }
  const lines = content.split('\n')
  return lines.map(tryFormatJson).join('\n')
}

function truncateToTail(content: string, maxLines: number): string {
  const lines = content.split('\n')
  if (lines.length <= maxLines) {
    return content
  }
  const omitted = lines.length - maxLines
  const tail = lines.slice(-maxLines).join('\n')
  return `... (${omitted} earlier lines omitted)\n${tail}`
}

export function OutputLine({
  content,
  verbose,
  isError,
  isWarning,
  maxLines = DEFAULT_MAX_LINES,
}: Props) {
  const expandShellOutput = useExpandShellOutput()
  const shouldShowFull = verbose || expandShellOutput

  const formatted = useMemo(() => {
    const stripped = stripAnsi(tryJsonFormatContent(content))
    if (shouldShowFull) {
      return stripped
    }
    return truncateToTail(stripped, maxLines)
  }, [content, shouldShowFull, maxLines])

  const color = isError ? c.error : isWarning ? c.warning : c.text

  return (
    <text selectable fg={color}>
      {formatted}
    </text>
  )
}

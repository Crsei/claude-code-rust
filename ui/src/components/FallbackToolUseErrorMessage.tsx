import React from 'react'
import { c } from '../theme.js'

/**
 * Generic tool-error rendering. Lite-native port of
 * `ui/examples/upstream-patterns/src/components/FallbackToolUseErrorMessage.tsx`.
 *
 * Upstream:
 * - extracts `<tool_use_error>` / `<error>` tags
 * - strips `<sandbox_violations>` payloads (for-the-model only)
 * - caps at 10 lines in non-verbose mode with a "+N lines (ctrl+o to see
 *   all)" tail
 * - strips ANSI underline escapes that arrive from the model
 *
 * cc-rust's tool errors come through the same `tool_result.output`
 * string path, so the same parsing applies. We reuse `c.error` for the
 * body, the existing dim tail, and `transcriptTitle`'s ctrl+o hint for
 * the "see all" footer.
 */

const MAX_RENDERED_LINES = 10

// Match upstream's extract-tag / strip-ansi-underline helpers inline —
// they are small and self-contained, and depending on the sample-tree
// copies would drag in the whole messages/ Ink runtime.
function extractTag(text: string, tag: string): string | null {
  const open = `<${tag}>`
  const close = `</${tag}>`
  const start = text.indexOf(open)
  if (start < 0) return null
  const end = text.indexOf(close, start + open.length)
  if (end < 0) return null
  return text.slice(start + open.length, end)
}

function removeSandboxViolationTags(text: string): string {
  return text.replace(/<sandbox_violations>[\s\S]*?<\/sandbox_violations>/g, '')
}

/**
 * Strip the ANSI `SGR 4` underline sequence Bash tool output sometimes
 * carries — upstream strips it because Ink's bold-dim rendering makes
 * underlined text look like a hyperlink that isn't one. We do the same
 * because OpenTUI's dim-text path has the same visual confusion.
 */
function stripUnderlineAnsi(text: string): string {
  return text.replace(/\x1b\[(?:0?4|24)m/g, '')
}

function countLines(text: string): number {
  let n = 1
  for (const ch of text) if (ch === '\n') n++
  return n
}

type Props = {
  /** The raw `tool_result.output` payload. */
  result: string | null | undefined
  /** When true, show the full message; otherwise cap at `MAX_RENDERED_LINES`. */
  verbose: boolean
  /** Optional hotkey label for the transcript shortcut — defaults to `Ctrl+O`. */
  transcriptShortcut?: string
}

export function FallbackToolUseErrorMessage({
  result,
  verbose,
  transcriptShortcut = 'Ctrl+O',
}: Props) {
  let error: string
  if (typeof result !== 'string' || !result) {
    error = 'Tool execution failed'
  } else {
    const extractedError = extractTag(result, 'tool_use_error') ?? result
    const withoutSandboxViolations = removeSandboxViolationTags(extractedError)
    const withoutErrorTags = withoutSandboxViolations.replace(/<\/?error>/g, '')
    const trimmed = withoutErrorTags.trim()
    if (!verbose && trimmed.includes('InputValidationError: ')) {
      error = 'Invalid tool parameters'
    } else if (trimmed.startsWith('Error: ') || trimmed.startsWith('Cancelled: ')) {
      error = trimmed
    } else {
      error = `Error: ${trimmed}`
    }
  }

  const plusLines = countLines(error) - MAX_RENDERED_LINES
  const body = stripUnderlineAnsi(
    verbose ? error : error.split('\n').slice(0, MAX_RENDERED_LINES).join('\n'),
  )

  return (
    <box flexDirection="column" width="100%" paddingX={1}>
      <text fg={c.error} selectable>
        {body}
      </text>
      {!verbose && plusLines > 0 && (
        <box flexDirection="row">
          <text fg={c.dim}>
            {'\u2026 +'}
            {plusLines} {plusLines === 1 ? 'line' : 'lines'} (
          </text>
          <text fg={c.dim}>
            <strong>{transcriptShortcut}</strong>
          </text>
          <text fg={c.dim}> to see all)</text>
        </box>
      )}
    </box>
  )
}

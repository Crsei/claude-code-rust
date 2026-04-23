import React from 'react'
import { useTerminalDimensions } from '@opentui/react'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `MarkdownTable`
 * (`ui/examples/upstream-patterns/src/components/MarkdownTable.tsx`).
 *
 * Upstream receives a `marked.Tokens.Table` token with pre-parsed header
 * + rows and renders a fully-bordered table, switching to a vertical
 * key-value layout for narrow widths. Lite does not import `marked`
 * (see `./Markdown.tsx` for the rationale), so this component takes a
 * plain `{ header: string[]; rows: string[][] }` shape and builds the
 * same horizontal / vertical fallback using OpenTUI's `<box>` primitives
 * for borders. The algorithm for column-width distribution follows
 * upstream:
 *  1. each column gets at least its longest-word width,
 *  2. extra space is distributed proportionally to each column's ideal
 *     width,
 *  3. if the table still does not fit, fall back to vertical key/value
 *     rendering.
 */

export type MarkdownTableToken = {
  header: string[]
  rows: string[][]
  /** Optional per-column alignment — matches `Tokens.Table['align']`. */
  align?: Array<'left' | 'center' | 'right' | null>
}

type Props = {
  token: MarkdownTableToken
  /** Override width (used by tests). */
  forceWidth?: number
}

const MIN_COLUMN_WIDTH = 3
const SAFETY_MARGIN = 4
const MAX_ROW_LINES = 4

function stringWidth(s: string): number {
  // Simple code-point width. Upstream uses `stringWidth` from @anthropic/ink
  // which handles East-Asian full-width glyphs; the Lite bundle does not
  // carry that helper, so we approximate with code-point count. For plain
  // ASCII (the common case) this matches exactly.
  return [...s].length
}

function padAligned(text: string, width: number, align: 'left' | 'center' | 'right'): string {
  const textWidth = stringWidth(text)
  const pad = Math.max(0, width - textWidth)
  if (pad === 0) return text
  if (align === 'right') return ' '.repeat(pad) + text
  if (align === 'center') {
    const left = Math.floor(pad / 2)
    return ' '.repeat(left) + text + ' '.repeat(pad - left)
  }
  return text + ' '.repeat(pad)
}

function wrapText(text: string, width: number, hard: boolean): string[] {
  if (width <= 0) return [text]
  const words = text.split(/\s+/).filter(Boolean)
  const lines: string[] = []
  let current = ''
  for (const word of words) {
    if (!current) {
      if (stringWidth(word) <= width) {
        current = word
      } else if (hard) {
        const chars = [...word]
        while (chars.length > 0) {
          lines.push(chars.splice(0, width).join(''))
        }
      } else {
        lines.push(word)
      }
      continue
    }
    if (stringWidth(current) + 1 + stringWidth(word) <= width) {
      current += ' ' + word
    } else {
      lines.push(current)
      if (stringWidth(word) <= width) {
        current = word
      } else if (hard) {
        const chars = [...word]
        while (chars.length > 0) {
          lines.push(chars.splice(0, width).join(''))
        }
        current = ''
      } else {
        current = word
      }
    }
  }
  if (current) lines.push(current)
  return lines.length > 0 ? lines : ['']
}

function getMinWidth(text: string): number {
  const words = text.split(/\s+/).filter(Boolean)
  if (words.length === 0) return MIN_COLUMN_WIDTH
  return Math.max(MIN_COLUMN_WIDTH, ...words.map(w => stringWidth(w)))
}

function getIdealWidth(text: string): number {
  return Math.max(MIN_COLUMN_WIDTH, stringWidth(text))
}

export function MarkdownTable({ token, forceWidth }: Props) {
  const { width: actualWidth } = useTerminalDimensions()
  const terminalWidth = forceWidth ?? actualWidth

  const numCols = token.header.length
  if (numCols === 0) return null

  const minWidths = token.header.map((header, col) => {
    let max = getMinWidth(header)
    for (const row of token.rows) {
      max = Math.max(max, getMinWidth(row[col] ?? ''))
    }
    return max
  })

  const idealWidths = token.header.map((header, col) => {
    let max = getIdealWidth(header)
    for (const row of token.rows) {
      max = Math.max(max, getIdealWidth(row[col] ?? ''))
    }
    return max
  })

  const borderOverhead = 1 + numCols * 3
  const availableWidth = Math.max(
    terminalWidth - borderOverhead - SAFETY_MARGIN,
    numCols * MIN_COLUMN_WIDTH,
  )
  const totalMin = minWidths.reduce((a, b) => a + b, 0)
  const totalIdeal = idealWidths.reduce((a, b) => a + b, 0)

  let columnWidths: number[]
  let needsHardWrap = false
  if (totalIdeal <= availableWidth) {
    columnWidths = idealWidths
  } else if (totalMin <= availableWidth) {
    const extra = availableWidth - totalMin
    const overflow = idealWidths.map((ideal, i) => ideal - minWidths[i]!)
    const overflowSum = overflow.reduce((a, b) => a + b, 0)
    columnWidths = minWidths.map((min, i) => {
      if (overflowSum === 0) return min
      return min + Math.floor((overflow[i]! / overflowSum) * extra)
    })
  } else {
    needsHardWrap = true
    const scale = availableWidth / totalMin
    columnWidths = minWidths.map(m => Math.max(MIN_COLUMN_WIDTH, Math.floor(m * scale)))
  }

  const rowLines = (cells: string[], isHeader: boolean) => {
    const wrapped = cells.map((cell, col) =>
      wrapText(cell, columnWidths[col]!, needsHardWrap),
    )
    const maxLines = Math.max(1, ...wrapped.map(lines => lines.length))
    const offsets = wrapped.map(lines => Math.floor((maxLines - lines.length) / 2))
    const output: string[] = []
    for (let lineIdx = 0; lineIdx < maxLines; lineIdx++) {
      let line = '\u2502'
      for (let col = 0; col < cells.length; col++) {
        const contentIdx = lineIdx - offsets[col]!
        const lines = wrapped[col]!
        const text = contentIdx >= 0 && contentIdx < lines.length ? lines[contentIdx]! : ''
        const width = columnWidths[col]!
        const align = isHeader ? 'center' : (token.align?.[col] ?? 'left') || 'left'
        line += ' ' + padAligned(text, width, align) + ' \u2502'
      }
      output.push(line)
    }
    return output
  }

  const maxRowLines = (() => {
    let max = 1
    for (let col = 0; col < token.header.length; col++) {
      max = Math.max(max, wrapText(token.header[col] ?? '', columnWidths[col]!, needsHardWrap).length)
    }
    for (const row of token.rows) {
      for (let col = 0; col < row.length; col++) {
        max = Math.max(max, wrapText(row[col] ?? '', columnWidths[col]!, needsHardWrap).length)
      }
    }
    return max
  })()

  const useVertical = maxRowLines > MAX_ROW_LINES

  if (useVertical) {
    return <VerticalTable token={token} terminalWidth={terminalWidth} />
  }

  const border = (kind: 'top' | 'mid' | 'bot') => {
    const parts: Record<'top' | 'mid' | 'bot', [string, string, string, string]> = {
      top: ['\u250C', '\u2500', '\u252C', '\u2510'],
      mid: ['\u251C', '\u2500', '\u253C', '\u2524'],
      bot: ['\u2514', '\u2500', '\u2534', '\u2518'],
    }
    const [left, mid, cross, right] = parts[kind]
    let line = left
    columnWidths.forEach((w, i) => {
      line += mid.repeat(w + 2)
      line += i < columnWidths.length - 1 ? cross : right
    })
    return line
  }

  const lines: string[] = []
  lines.push(border('top'))
  lines.push(...rowLines(token.header, true))
  lines.push(border('mid'))
  token.rows.forEach((row, i) => {
    lines.push(...rowLines(row, false))
    if (i < token.rows.length - 1) lines.push(border('mid'))
  })
  lines.push(border('bot'))

  // Safety fallback — same logic as upstream.
  const maxLineWidth = Math.max(...lines.map(l => stringWidth(l)))
  if (maxLineWidth > terminalWidth - SAFETY_MARGIN) {
    return <VerticalTable token={token} terminalWidth={terminalWidth} />
  }

  return (
    <box flexDirection="column">
      {lines.map((line, i) => (
        <text key={i} fg={c.text}>{line}</text>
      ))}
    </box>
  )
}

function VerticalTable({ token, terminalWidth }: { token: MarkdownTableToken; terminalWidth: number }) {
  const separatorWidth = Math.min(terminalWidth - 1, 40)
  const separator = '\u2500'.repeat(separatorWidth)
  const wrapIndent = '  '

  const items: React.ReactNode[] = []
  token.rows.forEach((row, rowIdx) => {
    if (rowIdx > 0) {
      items.push(
        <text key={`sep-${rowIdx}`} fg={c.dim}>{separator}</text>,
      )
    }
    row.forEach((cell, colIdx) => {
      const label = token.header[colIdx] ?? `Column ${colIdx + 1}`
      const value = cell.replace(/\s+/g, ' ').trim()
      const firstWidth = terminalWidth - stringWidth(label) - 3
      const restWidth = terminalWidth - wrapIndent.length - 1
      const first = wrapText(value, Math.max(firstWidth, 10), false)
      const firstLine = first[0] ?? ''
      let wrapped: string[]
      if (first.length <= 1 || restWidth <= firstWidth) {
        wrapped = first
      } else {
        const remaining = first.slice(1).map(l => l.trim()).join(' ')
        const rest = wrapText(remaining, restWidth, false)
        wrapped = [firstLine, ...rest]
      }
      items.push(
        <text key={`row-${rowIdx}-col-${colIdx}-first`}>
          <strong>{label}:</strong> {wrapped[0] ?? ''}
        </text>,
      )
      for (let i = 1; i < wrapped.length; i++) {
        const line = wrapped[i]
        if (!line || !line.trim()) continue
        items.push(
          <text key={`row-${rowIdx}-col-${colIdx}-cont-${i}`} fg={c.text}>
            {wrapIndent}{line}
          </text>,
        )
      }
    })
  })

  return <box flexDirection="column">{items}</box>
}

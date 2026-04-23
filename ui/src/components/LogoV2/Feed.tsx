import React from 'react'
import { c } from '../../theme.js'
import { stringWidth, truncateToWidth } from '../string-width.js'

/**
 * Single "feed" block inside the LogoV2 right column (Recent activity,
 * What's new, guest passes, etc.).
 *
 * OpenTUI-native port of the upstream `LogoV2/Feed`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/Feed.tsx`).
 * The upstream used Ink's `stringWidth`; the Lite port reuses the
 * local `string-width` helper for the same width-aware math.
 */

export type FeedLine = {
  text: string
  timestamp?: string
}

export type FeedConfig = {
  title: string
  lines: FeedLine[]
  footer?: string
  emptyMessage?: string
  customContent?: { content: React.ReactNode; width: number }
}

type FeedProps = {
  config: FeedConfig
  actualWidth: number
}

function truncate(text: string, width: number): string {
  if (width <= 0) return ''
  if (stringWidth(text) <= width) return text
  return truncateToWidth(text, Math.max(0, width - 1)) + '\u2026'
}

export function calculateFeedWidth(config: FeedConfig): number {
  const { title, lines, footer, emptyMessage, customContent } = config
  let maxWidth = stringWidth(title)

  if (customContent !== undefined) {
    maxWidth = Math.max(maxWidth, customContent.width)
  } else if (lines.length === 0 && emptyMessage) {
    maxWidth = Math.max(maxWidth, stringWidth(emptyMessage))
  } else {
    const gap = '  '
    const maxTimestampWidth = Math.max(
      0,
      ...lines.map(line => (line.timestamp ? stringWidth(line.timestamp) : 0)),
    )
    for (const line of lines) {
      const timestampWidth = maxTimestampWidth > 0 ? maxTimestampWidth : 0
      const lineWidth =
        stringWidth(line.text) +
        (timestampWidth > 0 ? timestampWidth + gap.length : 0)
      maxWidth = Math.max(maxWidth, lineWidth)
    }
  }
  if (footer) {
    maxWidth = Math.max(maxWidth, stringWidth(footer))
  }
  return maxWidth
}

export function Feed({ config, actualWidth }: FeedProps) {
  const { title, lines, footer, emptyMessage, customContent } = config
  const gap = '  '
  const maxTimestampWidth = Math.max(
    0,
    ...lines.map(line => (line.timestamp ? stringWidth(line.timestamp) : 0)),
  )

  return (
    <box flexDirection="column" width={actualWidth}>
      <text>
        <strong>
          <span fg={c.accent}>{title}</span>
        </strong>
      </text>
      {customContent ? (
        <>
          {customContent.content}
          {footer && (
            <text>
              <em>
                <span fg={c.dim}>{truncate(footer, actualWidth)}</span>
              </em>
            </text>
          )}
        </>
      ) : lines.length === 0 && emptyMessage ? (
        <text fg={c.dim}>{truncate(emptyMessage, actualWidth)}</text>
      ) : (
        <>
          {lines.map((line, index) => {
            const textWidth = Math.max(
              10,
              actualWidth -
                (maxTimestampWidth > 0 ? maxTimestampWidth + gap.length : 0),
            )
            return (
              <text key={index}>
                {maxTimestampWidth > 0 && (
                  <>
                    <span fg={c.dim}>
                      {(line.timestamp || '').padEnd(maxTimestampWidth)}
                    </span>
                    {gap}
                  </>
                )}
                <span>{truncate(line.text, textWidth)}</span>
              </text>
            )
          })}
          {footer && (
            <text>
              <em>
                <span fg={c.dim}>{truncate(footer, actualWidth)}</span>
              </em>
            </text>
          )}
        </>
      )}
    </box>
  )
}

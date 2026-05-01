import React from 'react'
import { useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'
import { formatWorkedDuration } from '../PromptInput/keys.js'

/**
 * Thinking header shown above assistant / streaming text when a segment
 * carries extended-thinking output.
 *
 * The default view hides the reasoning body and shows only elapsed thinking
 * time. The user can toggle the stored reasoning text with the configured
 * `chat:thinkingToggle` shortcut.
 */
type Props = {
  content: string
  durationMs?: number
}

export function ThinkingPreview({ content, durationMs }: Props) {
  const { showThinkingContent } = useAppState()
  const trimmed = content.trim()
  if (!trimmed) {
    return null
  }
  const duration = typeof durationMs === 'number'
    ? ` ${formatWorkedDuration(durationMs)}`
    : ''

  return (
    <box flexDirection="column" paddingX={1} marginBottom={1}>
      <text fg={c.dim}>
        <em>{`\u2234 Thinking${duration}`}</em>
      </text>
      {showThinkingContent && (
        <box paddingLeft={2}>
          <text fg={c.dim}>
            <em>{trimmed}</em>
          </text>
        </box>
      )}
    </box>
  )
}

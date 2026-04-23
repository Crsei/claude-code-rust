import React from 'react'
import { c } from '../theme.js'
import type { RawMessage } from '../store/message-model.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/MessageTimestamp.tsx`.
 *
 * Shows a dim `hh:mm am/pm` timestamp next to assistant text in
 * transcript mode. Returns `null` in prompt mode or when the message
 * has no timestamp / text payload — matching the upstream gate.
 */

type Props = {
  message: RawMessage
  isTranscriptMode: boolean
}

function hasText(message: RawMessage): boolean {
  if (message.content && message.content.length > 0) return true
  if (!message.contentBlocks) return false
  return message.contentBlocks.some(block => block.type === 'text')
}

export function MessageTimestamp({ message, isTranscriptMode }: Props) {
  const shouldShow =
    isTranscriptMode &&
    !!message.timestamp &&
    message.role === 'assistant' &&
    hasText(message)

  if (!shouldShow) {
    return null
  }

  const formatted = new Date(message.timestamp).toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
    hour12: true,
  })

  return (
    <box minWidth={formatted.length}>
      <text fg={c.dim}>{formatted}</text>
    </box>
  )
}

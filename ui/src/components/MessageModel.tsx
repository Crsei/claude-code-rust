import React from 'react'
import { c } from '../theme.js'
import type { RawMessage } from '../store/message-model.js'
import type { ViewMode } from '../keybindings.js'

/**
 * OpenTUI port of upstream `MessageModel`
 * (`ui/examples/upstream-patterns/src/components/MessageModel.tsx`).
 *
 * Displays the assistant model name next to a message when the caller is
 * in transcript mode. Lite's `RawMessage` carries `role` + `content` (plus
 * `contentBlocks`) but does not expose the upstream per-message `model`
 * field yet; when the backend adds it the component will render without
 * further changes. Until then it hides gracefully.
 */

type Props = {
  message: RawMessage & { model?: string }
  viewMode: ViewMode
}

export function MessageModel({ message, viewMode }: Props) {
  if (viewMode !== 'transcript') {
    return null
  }
  if (message.role !== 'assistant') {
    return null
  }
  const hasText =
    (message.contentBlocks?.some(block => block.type === 'text') ?? false) ||
    (typeof message.content === 'string' && message.content.trim().length > 0)
  if (!hasText) {
    return null
  }
  const model = message.model
  if (!model) {
    return null
  }
  return (
    <box minWidth={model.length + 2}>
      <text fg={c.dim}>{model}</text>
    </box>
  )
}

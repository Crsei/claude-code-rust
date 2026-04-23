import React from 'react'
import type { ViewMode } from '../keybindings.js'
import type { RenderItem } from '../store/message-model.js'
import { c } from '../theme.js'
import { Message, hasThinkingContent } from './Message.js'
import { MessageModel } from './MessageModel.js'

/**
 * OpenTUI port of upstream `MessageRow`
 * (`ui/examples/upstream-patterns/src/components/MessageRow.tsx`).
 *
 * Upstream wraps `Message` with:
 *   1. Optional transcript-mode metadata row (timestamp + model pill).
 *   2. An `OffscreenFreeze` to cache the rendered element once it has
 *      scrolled out of the viewport.
 *
 * Lite's `MessageList` drives the conversation through the `RenderItem`
 * pipeline and delegates leaves to `MessageBubble`. `MessageRow` re-hosts
 * that shape for callers that want the upstream API: it renders the
 * optional metadata row above `Message` and delegates body rendering to
 * the existing bubble pipeline. The OffscreenFreeze optimisation is
 * skipped because OpenTUI's `scrollbox` already clips off-screen
 * content, so revisiting a message does not re-run layout.
 */

export type MessageRowProps = {
  item: RenderItem
  viewMode: ViewMode
  /** Optional per-message model name — populated once the backend
   *  exposes it on `RawMessage`. Until then the metadata row only shows
   *  a timestamp when the item supplies one. */
  model?: string
  columns?: number
}

function formatTimestamp(ts: number): string {
  try {
    return new Date(ts).toLocaleString()
  } catch {
    return ''
  }
}

function MessageRowImpl({ item, viewMode, model, columns }: MessageRowProps) {
  const isTranscript = viewMode === 'transcript'
  const isAssistant = item.type === 'assistant_text' || item.type === 'streaming'
  const hasMetadata =
    isTranscript && isAssistant && (model || item.timestamp) && (item.type === 'assistant_text' || item.type === 'streaming')

  if (!hasMetadata) {
    return <Message item={item} viewMode={viewMode} />
  }

  return (
    <box flexDirection="column" width={columns ?? '100%'}>
      <box flexDirection="row" justifyContent="flex-end" marginTop={1}>
        {item.timestamp && (
          <box marginRight={1}>
            <text fg={c.dim}>{formatTimestamp(item.timestamp)}</text>
          </box>
        )}
        {model && (
          <MessageModel
            message={{
              id: item.id,
              role: 'assistant',
              content:
                item.type === 'assistant_text' || item.type === 'streaming'
                  ? item.content
                  : '',
              timestamp: item.timestamp,
              model,
            }}
            viewMode={viewMode}
          />
        )}
      </box>
      <Message item={item} viewMode={viewMode} />
    </box>
  )
}

export const MessageRow = React.memo(MessageRowImpl, (prev, next) => {
  if (prev.item !== next.item) return false
  if (prev.viewMode !== next.viewMode) return false
  if (prev.model !== next.model) return false
  if (prev.columns !== next.columns) return false
  return true
})

/**
 * Helper exported upstream for transcripts that hide all but the latest
 * thinking block. Preserved for API compatibility.
 */
export function shouldShowThinking(item: RenderItem): boolean {
  return hasThinkingContent(item)
}

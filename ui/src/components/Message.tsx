import React from 'react'
import type { ViewMode } from '../keybindings.js'
import type { RenderItem } from '../store/message-model.js'
import { MessageBubble } from './MessageBubble.js'

/**
 * OpenTUI port of upstream `Message`
 * (`ui/examples/upstream-patterns/src/components/Message.tsx`).
 *
 * Upstream's `Message` takes a `NormalizedUserMessage | AssistantMessage |
 * AttachmentMessage | SystemMessage | GroupedToolUseMessage |
 * CollapsedReadSearchGroup`, walks its content blocks, and dispatches to
 * per-type leaves under `./messages/*`. Lite does the same work through
 * `store/message-model.ts` (`RawMessage` → `RenderItem` via
 * `buildRenderItems`) and `MessageBubble` (one leaf per `RenderItem`
 * discriminant).
 *
 * Re-hosting `Message` as a thin adapter on top of that pipeline gives
 * callers that reference `Message.tsx` a working component without
 * pulling in upstream's Ink-specific state graph. The props shape is
 * reduced to what Lite actually has access to — `RenderItem` plus
 * `viewMode` — because Lite's upstream-style `NormalizedMessage` types
 * are not surfaced through IPC today.
 *
 * Upstream's niceties that require data not present in Lite's IPC
 * surface (per-message `model`, `timestamp` header on transcript-only
 * rows, CompactBoundary, AdvisorMessage, OffscreenFreeze cache) are
 * deliberately skipped; the per-type leaves under `./messages/` handle
 * what's exposed by `RenderItem`.
 */

export type MessageProps = {
  item: RenderItem
  viewMode: ViewMode
  /** Upstream parameters preserved for API compatibility — currently
   *  ignored because the Lite leaves consume them via the store. */
  addMargin?: boolean
  verbose?: boolean
  isTranscriptMode?: boolean
  isStatic?: boolean
}

function MessageImpl({ item, viewMode }: MessageProps) {
  return <MessageBubble item={item} viewMode={viewMode} />
}

export const Message = React.memo(MessageImpl, (prev, next) => {
  return (
    prev.item === next.item &&
    prev.viewMode === next.viewMode &&
    prev.verbose === next.verbose &&
    prev.isTranscriptMode === next.isTranscriptMode &&
    prev.isStatic === next.isStatic
  )
})

/**
 * Porting of upstream's `hasThinkingContent` helper. Works against any
 * object shaped like `RenderItem`, `RawMessage`, or upstream's
 * `NormalizedMessage`. Returns true if the object exposes either
 * thinking text or a `thinking` / `redacted_thinking` content block.
 */
export function hasThinkingContent(m: {
  type?: string
  role?: string
  thinking?: string
  message?: { content: Array<{ type: string }> }
  contentBlocks?: Array<{ type: string }>
}): boolean {
  if (typeof m.thinking === 'string' && m.thinking.trim().length > 0) {
    return true
  }
  const blocks = m.contentBlocks ?? m.message?.content
  if (!Array.isArray(blocks)) return false
  return blocks.some(
    block => block.type === 'thinking' || block.type === 'redacted_thinking',
  )
}

/**
 * Porting of upstream's `areMessagePropsEqual` for tests / direct memo
 * consumers. Matches the default `React.memo` comparator above.
 */
export function areMessagePropsEqual(prev: MessageProps, next: MessageProps): boolean {
  return (
    prev.item === next.item &&
    prev.viewMode === next.viewMode &&
    prev.verbose === next.verbose &&
    prev.isTranscriptMode === next.isTranscriptMode &&
    prev.isStatic === next.isStatic
  )
}

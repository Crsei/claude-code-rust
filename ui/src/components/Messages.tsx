import React, { useMemo } from 'react'
import type { ViewMode } from '../keybindings.js'
import { buildRenderItems, type RawMessage, type RenderItem } from '../store/message-model.js'
import { c } from '../theme.js'
import { MessageRow } from './MessageRow.js'

/**
 * OpenTUI port of upstream `Messages`
 * (`ui/examples/upstream-patterns/src/components/Messages.tsx`).
 *
 * Upstream is a ~1.1K-line orchestrator that:
 *  - normalizes the raw conversation buffer (collapse read/search groups,
 *    group tool uses, filter hidden messages, reorder syn messages), and
 *  - hands the resulting list to a virtualized list
 *    (`VirtualMessageList`) that streams ANSI into an Ink `<Static>`
 *    region.
 *
 * Lite already has that pipeline under a different shape:
 *   - `store/message-model.ts::buildRenderItems` applies the
 *     normalization + grouping (tool-group collapsing, streaming splice,
 *     etc.) against `RawMessage[]`.
 *   - `components/MessageList.tsx` renders the result inside an OpenTUI
 *     `<scrollbox>`, driven by the app store.
 *
 * This re-host provides the upstream call surface (`Messages` taking a
 * message list + view mode) so callers that reference `Messages.tsx`
 * receive the normalized list rendered with `MessageRow`. The scroll /
 * virtualization / logo-header concerns stay in `MessageList.tsx` —
 * this component is intentionally a composition point, not a runtime
 * replacement.
 */

export type MessagesProps = {
  messages: RawMessage[]
  viewMode: ViewMode
  /** Live streaming text for the current turn, if any. */
  streamingText?: string
  streamingThinking?: string
  /** Whether the conversation is currently streaming or waiting. Drives
   *  the `'running'` state on in-flight tool activities. */
  isBusy?: boolean
  /** Optional absolute width for the containing box. */
  columns?: number
}

export function Messages({
  messages,
  viewMode,
  streamingText,
  streamingThinking,
  isBusy,
  columns,
}: MessagesProps) {
  const items = useMemo(
    () =>
      buildRenderItems(messages, {
        viewMode,
        isBusy: !!isBusy,
        streamingText,
        streamingThinking,
      }),
    [isBusy, messages, streamingText, streamingThinking, viewMode],
  )

  if (items.length === 0) {
    return null
  }

  return (
    <box flexDirection="column" width={columns ?? '100%'} backgroundColor={c.bg}>
      {items.map(item => (
        <MessageRow key={item.id} item={item} viewMode={viewMode} columns={typeof columns === 'number' ? columns : undefined} />
      ))}
    </box>
  )
}

/**
 * Exported so test code can reuse the static-message heuristic. Mirrors
 * upstream's `shouldRenderStatically`: anything that has already
 * resolved (no tool activity still running, no streaming body) is safe
 * to render into upstream's `<Static>` region. In Lite everything lives
 * inside `<scrollbox>`, so this is advisory — callers use it when
 * deciding whether to skip memo bust on transient state changes.
 */
export function shouldRenderStatically(item: RenderItem): boolean {
  if (item.type === 'streaming') return false
  if (item.type === 'tool_activity') {
    return item.status !== 'running' && item.status !== 'pending'
  }
  if (item.type === 'tool_group') {
    return item.status !== 'running' && item.status !== 'pending'
  }
  return true
}

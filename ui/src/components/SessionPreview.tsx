import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'
import { MessageBubble } from './MessageBubble.js'
import { buildRenderItems, type RawMessage } from '../store/message-model.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/SessionPreview.tsx`.
 *
 * Shows the messages of a stored session in read-only form so the user
 * can decide whether to resume it. Upstream loads a full log off disk
 * (`loadFullLog(log)`) and renders them via `<Messages>`; the Lite
 * frontend already has a dispatcher (`MessageBubble`) that understands
 * `RenderItem`, so we take the messages directly and reuse that
 * pipeline.
 */

export type LogSummary = {
  id: string
  messageCount: number
  modified: number
  gitBranch?: string
  /** Already-loaded messages. When absent, the caller can lazy-load via
   *  `loadMessages`. */
  messages?: RawMessage[]
}

type Props = {
  log: LogSummary
  /** Lazy-loads the message list when `log.messages` is empty. */
  loadMessages?: (log: LogSummary) => Promise<RawMessage[]>
  onExit: () => void
  onSelect: (log: LogSummary) => void
}

function formatRelativeAgo(ts: number): string {
  const diff = Date.now() - ts
  if (diff < 0) return 'now'
  const minutes = Math.floor(diff / 60_000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.floor(hours / 24)
  if (days < 30) return `${days}d ago`
  const months = Math.floor(days / 30)
  if (months < 12) return `${months}mo ago`
  return `${Math.floor(months / 12)}y ago`
}

export function SessionPreview({ log, loadMessages, onExit, onSelect }: Props) {
  const [messages, setMessages] = useState<RawMessage[] | null>(
    log.messages ?? null,
  )
  const [loading, setLoading] = useState(messages === null)

  useEffect(() => {
    if (log.messages && log.messages.length > 0) {
      setMessages(log.messages)
      setLoading(false)
      return
    }
    if (!loadMessages) {
      setMessages(log.messages ?? [])
      setLoading(false)
      return
    }
    let cancelled = false
    setLoading(true)
    void loadMessages(log)
      .then(result => {
        if (!cancelled) {
          setMessages(result)
          setLoading(false)
        }
      })
      .catch(() => {
        if (!cancelled) {
          setMessages([])
          setLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [log, loadMessages])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'escape') {
      onExit()
      return
    }
    if (name === 'return' || name === 'enter') {
      onSelect(log)
    }
  })

  if (loading) {
    return (
      <box flexDirection="column" padding={1}>
        <Spinner label="Loading session…" />
        <text fg={c.dim}>Esc to cancel</text>
      </box>
    )
  }

  const items = messages
    ? buildRenderItems(messages, {
        viewMode: 'transcript',
        isBusy: false,
        streamingText: '',
        streamingThinking: '',
      })
    : []

  return (
    <box flexDirection="column">
      <scrollbox flexGrow={1} width="100%">
        {items.map(item => (
          <box key={item.id} width="100%">
            <MessageBubble item={item} viewMode="transcript" />
          </box>
        ))}
      </scrollbox>
      <box
        flexShrink={0}
        flexDirection="column"
        borderStyle="single"
        borderColor={c.dim}
        paddingLeft={2}
      >
        <text>
          {formatRelativeAgo(log.modified)} · {log.messageCount} messages
          {log.gitBranch ? ` · ${log.gitBranch}` : ''}
        </text>
        <text fg={c.dim}>Enter to resume · Esc to cancel</text>
      </box>
    </box>
  )
}

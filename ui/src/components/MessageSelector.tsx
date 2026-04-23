import React, { useCallback, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { RawMessage } from '../store/message-model.js'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * OpenTUI port of upstream `MessageSelector`
 * (`ui/examples/upstream-patterns/src/components/MessageSelector.tsx`).
 *
 * Upstream powers the "Rewind to…" pop-up — a list of prior user turns
 * the operator can restore (conversation only / code only / both / or
 * summarize from). It depends on several pieces that Lite does not have
 * surfaced through IPC yet:
 *   - `fileHistoryCanRestore` / `fileHistoryGetDiffStats` (rust-side
 *     file snapshot store),
 *   - `onSummarize` + compaction direction commands,
 *   - analytics / telemetry events.
 *
 * The re-host keeps the same public call surface (`messages` + callback
 * props) so the upstream rewind flow can be wired to the backend when
 * the commands land. Today the UI:
 *   - paginates user messages with ↑ / ↓ / home / end,
 *   - shows a focused "restore options" step once a message is picked,
 *   - invokes the appropriate callback on confirm.
 *
 * The `onRestoreCode` / `onSummarize` callbacks are optional — when the
 * parent does not provide them, the corresponding options are hidden.
 */

export type PartialCompactDirection = 'from' | 'up_to'

export type MessageSelectorProps = {
  messages: RawMessage[]
  onPreRestore?: () => void
  onRestoreMessage: (message: RawMessage) => Promise<void> | void
  onRestoreCode?: (message: RawMessage) => Promise<void> | void
  onSummarize?: (
    message: RawMessage,
    feedback?: string,
    direction?: PartialCompactDirection,
  ) => Promise<void> | void
  onClose: () => void
  /** Skip the pick-list and jump straight to the confirm step. */
  preselectedMessage?: RawMessage
}

type RestoreOption =
  | 'both'
  | 'conversation'
  | 'code'
  | 'summarize'
  | 'nevermind'

const MAX_VISIBLE = 7

function userMessageSelectable(m: RawMessage): boolean {
  if (m.role !== 'user') return false
  if (!m.content?.trim()) return false
  // Hide XML-wrapped system echoes (command expansions, bash stdout, …).
  return !m.content.trimStart().startsWith('<')
}

function shortTitle(message: RawMessage, maxWidth = 80): string {
  const compact = message.content.replace(/\s+/g, ' ').trim()
  if (compact.length <= maxWidth) return compact
  return compact.slice(0, maxWidth - 1) + '\u2026'
}

export function MessageSelector({
  messages,
  onPreRestore,
  onRestoreMessage,
  onRestoreCode,
  onSummarize,
  onClose,
  preselectedMessage,
}: MessageSelectorProps) {
  const candidates = useMemo(
    () => messages.filter(userMessageSelectable),
    [messages],
  )
  const [selectedIndex, setSelectedIndex] = useState(
    Math.max(0, candidates.length - 1),
  )
  const [messageToRestore, setMessageToRestore] = useState<
    RawMessage | undefined
  >(preselectedMessage)
  const [isRestoring, setIsRestoring] = useState(false)
  const [error, setError] = useState<string | undefined>()
  const [restoreOption, setRestoreOption] = useState<RestoreOption>('both')

  const availableOptions = useMemo<Array<{ value: RestoreOption; label: string }>>(
    () => {
      const list: Array<{ value: RestoreOption; label: string }> = []
      if (onRestoreCode) {
        list.push({ value: 'both', label: 'Restore code and conversation' })
        list.push({ value: 'conversation', label: 'Restore conversation' })
        list.push({ value: 'code', label: 'Restore code' })
      } else {
        list.push({ value: 'conversation', label: 'Restore conversation' })
      }
      if (onSummarize) {
        list.push({ value: 'summarize', label: 'Summarize from here' })
      }
      list.push({ value: 'nevermind', label: 'Never mind' })
      return list
    },
    [onRestoreCode, onSummarize],
  )
  const [optionIndex, setOptionIndex] = useState(0)

  const firstVisible = Math.max(
    0,
    Math.min(
      selectedIndex - Math.floor(MAX_VISIBLE / 2),
      candidates.length - MAX_VISIBLE,
    ),
  )

  const runRestore = useCallback(
    async (message: RawMessage, option: RestoreOption) => {
      if (option === 'nevermind') {
        if (preselectedMessage) onClose()
        else setMessageToRestore(undefined)
        return
      }
      onPreRestore?.()
      setIsRestoring(true)
      setError(undefined)
      try {
        if (option === 'summarize' && onSummarize) {
          await onSummarize(message)
        } else {
          if ((option === 'code' || option === 'both') && onRestoreCode) {
            await onRestoreCode(message)
          }
          if (option === 'conversation' || option === 'both') {
            await onRestoreMessage(message)
          }
        }
        onClose()
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err))
      } finally {
        setIsRestoring(false)
      }
    },
    [
      onClose,
      onPreRestore,
      onRestoreCode,
      onRestoreMessage,
      onSummarize,
      preselectedMessage,
    ],
  )

  useKeyboard(event => {
    if (event.eventType === 'release' || isRestoring) return
    const key = event.name

    if (messageToRestore) {
      if (key === 'escape') {
        if (preselectedMessage) onClose()
        else setMessageToRestore(undefined)
        return
      }
      if (key === 'up') {
        setOptionIndex(i => (i - 1 + availableOptions.length) % availableOptions.length)
        return
      }
      if (key === 'down' || key === 'tab') {
        setOptionIndex(i => (i + 1) % availableOptions.length)
        return
      }
      if (key === 'return' || key === 'enter') {
        const opt = availableOptions[optionIndex]
        if (opt) {
          setRestoreOption(opt.value)
          void runRestore(messageToRestore, opt.value)
        }
      }
      return
    }

    if (key === 'escape') {
      onClose()
      return
    }
    if (candidates.length === 0) return
    if (key === 'up') {
      setSelectedIndex(i => Math.max(0, i - 1))
      return
    }
    if (key === 'down') {
      setSelectedIndex(i => Math.min(candidates.length - 1, i + 1))
      return
    }
    if (key === 'home') {
      setSelectedIndex(0)
      return
    }
    if (key === 'end') {
      setSelectedIndex(candidates.length - 1)
      return
    }
    if (key === 'return' || key === 'enter') {
      const chosen = candidates[selectedIndex]
      if (chosen) {
        setMessageToRestore(chosen)
        setOptionIndex(0)
      }
    }
  })

  if (candidates.length === 0 && !preselectedMessage) {
    return (
      <box
        flexDirection="column"
        borderStyle="rounded"
        borderColor={c.accent}
        paddingX={2}
        paddingY={1}
        title="Rewind"
        titleAlignment="center"
      >
        <text>Nothing to rewind to yet.</text>
      </box>
    )
  }

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
      title="Rewind"
      titleAlignment="center"
    >
      {error && (
        <box marginBottom={1}>
          <text fg={c.error}>Error: {error}</text>
        </box>
      )}

      {!messageToRestore && (
        <>
          <text>
            {onRestoreCode
              ? 'Restore the code and/or conversation to the point before…'
              : 'Restore and fork the conversation to the point before…'}
          </text>
          <box flexDirection="column" marginTop={1}>
            {candidates.slice(firstVisible, firstVisible + MAX_VISIBLE).map((msg, i) => {
              const optionIndex = firstVisible + i
              const isSelected = optionIndex === selectedIndex
              return (
                <text key={msg.id}>
                  <span fg={isSelected ? c.accent : c.dim}>
                    {isSelected ? '\u25B8 ' : '  '}
                  </span>
                  <span fg={isSelected ? c.textBright : c.text}>
                    {shortTitle(msg)}
                  </span>
                </text>
              )
            })}
          </box>
          <box marginTop={1}>
            <text fg={c.dim}>
              <em>↑↓ navigate · Enter pick · Esc close</em>
            </text>
          </box>
        </>
      )}

      {messageToRestore && (
        <>
          <text>
            Confirm you want to restore the conversation to the point before you sent this message:
          </text>
          <box
            flexDirection="column"
            marginTop={1}
            paddingLeft={1}
            borderStyle="single"
            border={['left']}
            borderColor={c.dim}
          >
            <text fg={c.text}>{shortTitle(messageToRestore, 120)}</text>
          </box>

          {isRestoring && restoreOption === 'summarize' ? (
            <box flexDirection="row" marginTop={1}>
              <Spinner />
              <text>{' Summarizing…'}</text>
            </box>
          ) : (
            <box flexDirection="column" marginTop={1}>
              {availableOptions.map((opt, i) => {
                const focused = i === optionIndex
                return (
                  <text key={opt.value}>
                    <span fg={focused ? c.accent : c.dim}>
                      {focused ? '\u25B8 ' : '  '}
                    </span>
                    <span fg={focused ? c.textBright : c.text}>
                      {opt.label}
                    </span>
                  </text>
                )
              })}
            </box>
          )}
          <box marginTop={1}>
            <text fg={c.dim}>
              <em>Enter confirm · Esc back</em>
            </text>
          </box>
        </>
      )}
    </box>
  )
}

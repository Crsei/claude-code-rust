import React, { useCallback, useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useAppState } from '../store/app-store.js'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/ExportDialog.tsx`.
 *
 * Upstream lets the user export the current conversation to the
 * clipboard or to a file in CWD, driving the selection through `Select`
 * + `TextInput`. cc-rust has neither helper in the active tree; we
 * inline:
 *
 *  - A two-row option select (keyboard-driven, no Ink primitives).
 *  - A text-entry row that edits a mutable filename via `useKeyboard`
 *    — enough for the small filename editor this dialog needs without
 *    pulling in the full `PromptInput/`.
 *
 * The IO side stays pluggable: the parent passes `onCopyToClipboard`
 * and `onSaveToFile` callbacks. That keeps the dialog testable and
 * lets the host route through whatever clipboard / FS helpers are
 * safe in that environment (Bun's `Bun.write`, the backend's
 * `write_file` IPC, etc.).
 */

type ExportOption = 'clipboard' | 'file'

type ExportResult = { success: boolean; message: string }

type Props = {
  content: string
  /** Defaults to `conversation.txt`. Used to prime the filename input. */
  defaultFilename?: string
  onDone: (result: ExportResult) => void
  /**
   * Called when the user picks "Copy to clipboard". Should return a
   * `Promise<ExportResult>` so we can surface IO errors back to the
   * dialog. Defaults to Bun's clipboard integration via `Bun.write`
   * when the runtime exposes it — otherwise the caller MUST pass a
   * handler or the dialog will fall back to a generic failure message.
   */
  onCopyToClipboard?: (content: string) => Promise<ExportResult>
  /** Called when the user picks "Save to file" and submits a filename. */
  onSaveToFile?: (content: string, filename: string, cwd: string) => Promise<ExportResult>
}

const OPTIONS: Array<{
  value: ExportOption
  label: string
  description: string
  hotkey: string
}> = [
  {
    value: 'clipboard',
    label: 'Copy to clipboard',
    description: 'Copy the conversation to your system clipboard',
    hotkey: 'c',
  },
  {
    value: 'file',
    label: 'Save to file',
    description: 'Save the conversation to a file in the current directory',
    hotkey: 'f',
  },
]

async function defaultCopyToClipboard(content: string): Promise<ExportResult> {
  try {
    const maybeBun = (globalThis as unknown as { Bun?: { write: (target: unknown, data: unknown) => Promise<number> } }).Bun
    if (maybeBun && typeof maybeBun.write === 'function') {
      // `Bun.write` with the clipboard path is the shortest correct
      // integration in the Bun runtime that hosts the frontend.
      // Terminals outside that runtime should pass `onCopyToClipboard`
      // explicitly.
      await maybeBun.write('/dev/clipboard', content)
      return { success: true, message: 'Conversation copied to clipboard' }
    }
  } catch (error) {
    return {
      success: false,
      message: `Failed to copy to clipboard: ${error instanceof Error ? error.message : 'Unknown error'}`,
    }
  }
  return {
    success: false,
    message: 'Clipboard access is not available in this runtime',
  }
}

async function defaultSaveToFile(
  content: string,
  filename: string,
  cwd: string,
): Promise<ExportResult> {
  try {
    const maybeBun = (globalThis as unknown as { Bun?: { write: (target: unknown, data: unknown) => Promise<number> } }).Bun
    const targetPath = `${cwd.replace(/[/\\]+$/, '')}/${filename}`
    if (maybeBun && typeof maybeBun.write === 'function') {
      await maybeBun.write(targetPath, content)
      return { success: true, message: `Conversation exported to: ${targetPath}` }
    }
  } catch (error) {
    return {
      success: false,
      message: `Failed to export conversation: ${error instanceof Error ? error.message : 'Unknown error'}`,
    }
  }
  return {
    success: false,
    message: 'File writes are not available in this runtime',
  }
}

function normalizeFilename(filename: string): string {
  if (filename.endsWith('.txt')) return filename
  return filename.replace(/\.[^.]+$/, '') + '.txt'
}

export function ExportDialog({
  content,
  defaultFilename = 'conversation.txt',
  onDone,
  onCopyToClipboard = defaultCopyToClipboard,
  onSaveToFile = defaultSaveToFile,
}: Props) {
  const cwd = useAppState().cwd
  const [selected, setSelected] = useState(0)
  const [showFilenameInput, setShowFilenameInput] = useState(false)
  const [filename, setFilename] = useState(defaultFilename)
  const safeIndex = Math.max(0, Math.min(selected, OPTIONS.length - 1))

  const handleCopy = useCallback(async () => {
    const result = await onCopyToClipboard(content)
    onDone(result)
  }, [content, onCopyToClipboard, onDone])

  const handleSave = useCallback(async () => {
    const result = await onSaveToFile(content, normalizeFilename(filename), cwd)
    onDone(result)
  }, [content, cwd, filename, onDone, onSaveToFile])

  const cancel = useCallback(() => {
    if (showFilenameInput) {
      setShowFilenameInput(false)
    } else {
      onDone({ success: false, message: 'Export cancelled' })
    }
  }, [onDone, showFilenameInput])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const input = (seq ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (showFilenameInput) {
      // Filename editor — most printable keys go through to the string.
      if (name === 'escape') {
        cancel()
        return
      }
      if (name === 'return' || name === 'enter') {
        void handleSave()
        return
      }
      if (name === 'backspace' || name === 'delete') {
        setFilename(current => current.slice(0, Math.max(0, current.length - 1)))
        return
      }
      if (seq && seq.length === 1 && seq.charCodeAt(0) >= 0x20) {
        setFilename(current => current + seq)
        return
      }
      // Allow single-letter keys too (y/n/c/f) since some terminals
      // emit them via `name` rather than `sequence`.
      if (!seq && name && name.length === 1 && name.charCodeAt(0) >= 0x20) {
        setFilename(current => current + name)
      }
      return
    }

    if (name === 'escape') {
      cancel()
      return
    }
    if (input === 'c') {
      void handleCopy()
      return
    }
    if (input === 'f') {
      setShowFilenameInput(true)
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setSelected(Math.min(OPTIONS.length - 1, safeIndex + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const opt = OPTIONS[safeIndex]
      if (!opt) return
      if (opt.value === 'clipboard') void handleCopy()
      else setShowFilenameInput(true)
    }
  })

  // Cursor blink — small marker tied to a render tick. Avoids pulling
  // a dedicated cursor component.
  const [blink, setBlink] = useState(true)
  useEffect(() => {
    if (!showFilenameInput) return undefined
    const timer = setInterval(() => setBlink(prev => !prev), 500)
    return () => clearInterval(timer)
  }, [showFilenameInput])

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.info}
      title="Export Conversation"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      {!showFilenameInput ? (
        <>
          <text fg={c.dim}>Select export method:</text>
          <box marginTop={1} flexDirection="column">
            {OPTIONS.map((opt, i) => {
              const isSelected = i === safeIndex
              return (
                <box key={opt.value} flexDirection="column" marginBottom={1}>
                  <box flexDirection="row">
                    <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                      <strong>{` ${opt.label} `}</strong>
                    </text>
                    <text fg={c.dim}> ({opt.hotkey})</text>
                  </box>
                  <text fg={c.dim} selectable>
                    {' '}
                    {opt.description}
                  </text>
                </box>
              )
            })}
          </box>
          <box marginTop={1}>
            <text>
              <em>
                <span fg={c.dim}>Up/Down · Enter to pick · Esc to cancel</span>
              </em>
            </text>
          </box>
        </>
      ) : (
        <>
          <text fg={c.text}>Enter filename:</text>
          <box marginTop={1} flexDirection="row" gap={1}>
            <text fg={c.info}>{'>'}</text>
            <text>
              {filename}
              <span fg={c.dim}>{blink ? '_' : ' '}</span>
            </text>
          </box>
          <box marginTop={1}>
            <text>
              <em>
                <span fg={c.dim}>Enter to save · Esc to go back</span>
              </em>
            </text>
          </box>
        </>
      )}
    </box>
  )
}

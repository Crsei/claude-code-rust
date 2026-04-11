import React, { useCallback, useEffect, useRef, useState } from 'react'
import { Box, Text, type ClickEvent, useAnimationFrame, useApp, useInput } from '../compat/ink-compat.js'
import { useBackend } from '../ipc/context.js'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import { VimState } from '../vim/index.js'

/** Minimum chars in a single input event to detect as paste */
const PASTE_DETECT_CHARS = 100
/** Minimum text length to show compact paste display */
const PASTE_COMPACT_CHARS = 200

export function isPasteInput(inputLength: number): boolean {
  return inputLength >= PASTE_DETECT_CHARS
}

export function formatPasteSize(text: string): string {
  const bytes = Buffer.byteLength(text, 'utf8')
  const kb = bytes / 1024
  if (kb < 10) {
    const rounded = Number(kb.toFixed(1))
    return `pasted text ${rounded}kb`
  }
  return `pasted text ${Math.round(kb)}kb`
}

function formatWorkedDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`
  }
  return `${seconds}s`
}

function buildSeparatorWithLabel(label: string, width: number): string {
  const safeWidth = Math.max(24, width)
  const wrappedLabel = `<${label}>`
  const rightTail = 3
  const minLeft = 1
  const leftLen = Math.max(minLeft, safeWidth - wrappedLabel.length - rightTail)
  return `${'\u2500'.repeat(leftLen)}${wrappedLabel}${'\u2500'.repeat(rightTail)}`
}

interface InputPromptProps {
  isActive?: boolean
  onActivate?: () => void
}

export function InputPrompt({ isActive = true, onActivate }: InputPromptProps) {
  const [text, setText] = useState('')
  const [cursorPos, setCursorPos] = useState(0)
  const [undoStack, setUndoStack] = useState<Array<{ text: string; cursor: number }>>([])
  const [isPasted, setIsPasted] = useState(false)
  const backend = useBackend()
  const { isStreaming, isWaiting, inputHistory, historyIndex, vimEnabled } = useAppState()
  const dispatch = useAppDispatch()
  const { exit } = useApp()
  const vimRef = useRef<VimState>(new VimState())
  const isBusy = isWaiting || isStreaming
  const [tickRef, time] = useAnimationFrame(isBusy ? 250 : null)
  const busyStartedAtRef = useRef<number | null>(null)
  const [lastWorkedMs, setLastWorkedMs] = useState(0)
  const inputActive = isActive

  useEffect(() => {
    if (isBusy) {
      if (busyStartedAtRef.current === null) {
        busyStartedAtRef.current = time
      }
      return
    }

    if (busyStartedAtRef.current !== null) {
      setLastWorkedMs(Math.max(0, time - busyStartedAtRef.current))
      busyStartedAtRef.current = null
    }
  }, [isBusy, time])

  const vim = vimRef.current
  if (vimEnabled && !vim.enabled) {
    vim.enable()
    dispatch({ type: 'SET_VIM_MODE', mode: vim.indicator })
  } else if (!vimEnabled && vim.enabled) {
    vim.disable()
  }

  const saveUndo = useCallback(() => {
    setUndoStack(stack => [...stack.slice(-50), { text, cursor: cursorPos }])
  }, [text, cursorPos])

  const activateInput = useCallback(() => {
    onActivate?.()

    // Clicking the composer while empty loads the latest prompt for quick recall.
    if (!isBusy && text.length === 0 && historyIndex === -1 && inputHistory.length > 0) {
      const idx = inputHistory.length - 1
      const latest = inputHistory[idx] || ''
      dispatch({ type: 'SET_HISTORY_INDEX', index: idx })
      setText(latest)
      setCursorPos(latest.length)
    }
  }, [dispatch, historyIndex, inputHistory, isBusy, onActivate, text.length])

  const submit = useCallback(() => {
    const trimmed = text.trim()
    if (!trimmed) return

    const id = `user-${Date.now()}`
    dispatch({ type: 'ADD_USER_MESSAGE', id, text: trimmed })
    dispatch({ type: 'PUSH_HISTORY', text: trimmed })

    if (trimmed.startsWith('/')) {
      backend.send({ type: 'slash_command', raw: trimmed })
    } else {
      backend.send({ type: 'submit_prompt', text: trimmed, id })
    }

    setText('')
    setCursorPos(0)
    setUndoStack([])
    setIsPasted(false)
  }, [backend, dispatch, text])

  const navigateHistoryUp = useCallback(() => {
    if (inputHistory.length === 0) return false
    const nextIdx = historyIndex === -1 ? inputHistory.length - 1 : Math.max(0, historyIndex - 1)
    dispatch({ type: 'SET_HISTORY_INDEX', index: nextIdx })
    const historyText = inputHistory[nextIdx] || ''
    setText(historyText)
    setCursorPos(historyText.length)
    return true
  }, [dispatch, historyIndex, inputHistory])

  const navigateHistoryDown = useCallback(() => {
    if (historyIndex === -1) return false
    const nextIdx = historyIndex + 1
    if (nextIdx >= inputHistory.length) {
      dispatch({ type: 'SET_HISTORY_INDEX', index: -1 })
      setText('')
      setCursorPos(0)
      return true
    }

    dispatch({ type: 'SET_HISTORY_INDEX', index: nextIdx })
    const historyText = inputHistory[nextIdx] || ''
    setText(historyText)
    setCursorPos(historyText.length)
    return true
  }, [dispatch, historyIndex, inputHistory])

  useInput((input, key, event) => {
    if (isStreaming && key.ctrl && input === 'c') {
      backend.send({ type: 'abort_query' })
      return
    }

    if (key.ctrl && input === 'd') {
      backend.send({ type: 'quit' })
      exit()
      return
    }

    if (key.ctrl && input === 'g') {
      vim.toggle()
      dispatch({ type: 'TOGGLE_VIM' })
      return
    }

    if (!inputActive || isBusy) {
      return
    }

    if (key.wheelUp && !key.ctrl && !key.meta) {
      if (navigateHistoryUp()) event.stopImmediatePropagation()
      return
    }

    if (key.wheelDown && !key.ctrl && !key.meta) {
      if (navigateHistoryDown()) event.stopImmediatePropagation()
      return
    }

    if (vim.enabled) {
      const action = vim.handleKey(input, key, text, cursorPos)
      dispatch({ type: 'SET_VIM_MODE', mode: vim.indicator })

      switch (action.type) {
        case 'none':
          return
        case 'passthrough':
          break
        case 'submit':
          submit()
          return
        case 'switch_mode':
          return
        case 'move_cursor':
          setCursorPos(action.pos)
          return
        case 'delete': {
          saveUndo()
          const next = text.slice(0, action.start) + text.slice(action.end)
          setText(next)
          setCursorPos(Math.min(action.start, next.length))
          return
        }
        case 'delete_line':
          saveUndo()
          vim.register = text
          setText('')
          setCursorPos(0)
          return
        case 'yank':
          vim.register = text.slice(action.start, action.end)
          return
        case 'yank_line':
          return
        case 'paste': {
          saveUndo()
          const next = text.slice(0, cursorPos) + action.text + text.slice(cursorPos)
          setText(next)
          setCursorPos(cursorPos + action.text.length)
          return
        }
        case 'insert_char': {
          saveUndo()
          const next = text.slice(0, cursorPos) + action.char + text.slice(cursorPos)
          setText(next)
          setCursorPos(cursorPos + action.char.length)
          return
        }
        case 'undo': {
          const prev = undoStack[undoStack.length - 1]
          if (prev) {
            setUndoStack(stack => stack.slice(0, -1))
            setText(prev.text)
            setCursorPos(prev.cursor)
          }
          return
        }
      }
    }

    if (key.return) {
      submit()
      return
    }

    if (key.backspace) {
      if (cursorPos > 0) {
        setText(t => t.slice(0, cursorPos - 1) + t.slice(cursorPos))
        setCursorPos(p => p - 1)
      }
      return
    }

    if (key.delete) {
      if (cursorPos < text.length) {
        setText(t => t.slice(0, cursorPos) + t.slice(cursorPos + 1))
      }
      return
    }

    if (key.leftArrow) {
      setCursorPos(p => Math.max(0, p - 1))
      return
    }

    if (key.rightArrow) {
      setCursorPos(p => Math.min(text.length, p + 1))
      return
    }

    if (key.ctrl && input === 'a') {
      setCursorPos(0)
      return
    }

    if (key.ctrl && input === 'e') {
      setCursorPos(text.length)
      return
    }

    if (key.ctrl && input === 'u') {
      setText('')
      setCursorPos(0)
      setIsPasted(false)
      return
    }

    if (key.ctrl && input === 'k') {
      setText(t => t.slice(0, cursorPos))
      return
    }

    if (key.ctrl && input === 'w') {
      const before = text.slice(0, cursorPos)
      const after = text.slice(cursorPos)
      const nextBefore = before.replace(/\S+\s*$/, '')
      setText(nextBefore + after)
      setCursorPos(nextBefore.length)
      return
    }

    if (key.upArrow && !key.ctrl && !key.meta) {
      navigateHistoryUp()
      return
    }

    if (key.downArrow && !key.ctrl && !key.meta) {
      navigateHistoryDown()
      return
    }

    if (input && !key.ctrl && !key.meta) {
      if (isPasteInput(input.length)) {
        setIsPasted(true)
      }
      setText(t => t.slice(0, cursorPos) + input + t.slice(cursorPos))
      setCursorPos(p => p + input.length)
    }
  })

  const showPasteCompact = isPasted && text.length >= PASTE_COMPACT_CHARS
  const before = text.slice(0, cursorPos)
  const cursorChar = cursorPos < text.length ? text[cursorPos] : ' '
  const after = cursorPos < text.length ? text.slice(cursorPos + 1) : ''
  const workingType = isStreaming ? 'reasoning' : isWaiting ? 'thinking' : 'input'
  const workedMs = isBusy && busyStartedAtRef.current !== null
    ? Math.max(0, time - busyStartedAtRef.current)
    : lastWorkedMs
  const workedLabel = `\u273b Worked for ${formatWorkedDuration(workedMs)}`
  const separatorWidth = Math.max(24, (process.stdout.columns ?? 80) - 2)
  const topSeparator = buildSeparatorWithLabel(workingType, separatorWidth)
  const bottomSeparator = '\u2500'.repeat(separatorWidth)

  return (
    <Box
      paddingX={1}
      onClick={(event: ClickEvent) => {
        activateInput()
        event.stopImmediatePropagation()
      }}
      ref={tickRef}
    >
      <Box flexDirection="column">
        <Text color="ansi:blackBright">{workedLabel}</Text>
        <Text color="ansi:blackBright">{topSeparator}</Text>
        <Box flexDirection="row">
          <Text bold color={inputActive ? 'ansi:whiteBright' : 'ansi:blackBright'}>{'\u276f '}</Text>
          {isBusy ? (
            <Text dim>{showPasteCompact ? formatPasteSize(text) : text}</Text>
          ) : showPasteCompact ? (
            <Text color="ansi:yellowBright">{formatPasteSize(text)}</Text>
          ) : (
            <>
              <Text>{before}</Text>
              <Text inverse>{cursorChar}</Text>
              <Text>{after}</Text>
            </>
          )}
        </Box>
        <Text color="ansi:blackBright">{bottomSeparator}</Text>
      </Box>
    </Box>
  )
}

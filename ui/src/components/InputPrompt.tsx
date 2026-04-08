import React, { useCallback, useRef, useState } from 'react'
import { Box, Text, type ClickEvent, useAnimationFrame, useApp, useInput } from 'ink-terminal'
import { useBackend } from '../ipc/context.js'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import { VimState } from '../vim/index.js'

const FRAMES = ['\u280b', '\u2819', '\u2839', '\u2838', '\u283c', '\u2834', '\u2826', '\u2827', '\u2807', '\u280f']

interface InputPromptProps {
  isActive?: boolean
  onActivate?: () => void
}

export function InputPrompt({ isActive = true, onActivate }: InputPromptProps) {
  const [text, setText] = useState('')
  const [cursorPos, setCursorPos] = useState(0)
  const [undoStack, setUndoStack] = useState<Array<{ text: string; cursor: number }>>([])
  const backend = useBackend()
  const { isStreaming, isWaiting, streamingText, inputHistory, historyIndex, vimEnabled } = useAppState()
  const dispatch = useAppDispatch()
  const { exit } = useApp()
  const vimRef = useRef<VimState>(new VimState())
  const isBusy = isWaiting || isStreaming
  const [spinnerRef, time] = useAnimationFrame(isBusy ? 80 : null)
  const inputActive = isActive

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
      setText(t => t.slice(0, cursorPos) + input + t.slice(cursorPos))
      setCursorPos(p => p + input.length)
    }
  })

  const before = text.slice(0, cursorPos)
  const cursorChar = cursorPos < text.length ? text[cursorPos] : '_'
  const after = cursorPos < text.length ? text.slice(cursorPos + 1) : ''
  const prompt = vim.enabled
    ? (vim.mode === 'normal' ? '[N]' : vim.mode === 'visual' ? '[V]' : '[I]')
    : '[>]'
  const spinnerFrame = FRAMES[Math.floor(time / 80) % FRAMES.length]
  const statusLabel = isStreaming && streamingText ? 'Reasoning...' : 'Thinking...'
  const historyLabel = inputHistory.length === 0
    ? 'History: empty'
    : `History: ${historyIndex === -1 ? inputHistory.length : historyIndex + 1}/${inputHistory.length}`
  const focusLabel = inputActive ? 'Input focus' : 'Click composer to focus input'
  const borderColor = isBusy
    ? 'ansi:cyanBright'
    : inputActive
      ? 'ansi:yellowBright'
      : 'ansi:blackBright'

  let body: React.ReactNode
  if (isBusy) {
    body = (
      <Box ref={spinnerRef} flexDirection="row">
        <Text color="ansi:cyan">{spinnerFrame} </Text>
        <Text color="ansi:cyan">{statusLabel}</Text>
        <Text dim> Ctrl+C to abort</Text>
      </Box>
    )
  } else {
    body = (
      <Box flexDirection="row">
        <Text bold color={inputActive ? 'ansi:yellowBright' : 'ansi:blackBright'}>
          {prompt} 
        </Text>
        <Text>{before}</Text>
        <Text inverse>{cursorChar}</Text>
        <Text>{after}</Text>
        {text.length === 0 && <Text dim> Type a message. Enter to send.</Text>}
      </Box>
    )
  }

  return (
    <Box
      paddingX={1}
      onClick={(event: ClickEvent) => {
        activateInput()
        event.stopImmediatePropagation()
      }}
    >
      <Box flexDirection="column" borderStyle="round" borderColor={borderColor as any} paddingX={1}>
        {body}
        {!isBusy && (
          <Box>
            <Text dim>{focusLabel} | {historyLabel} | wheel=history</Text>
          </Box>
        )}
      </Box>
    </Box>
  )
}

import React, { useState, useCallback, useRef } from 'react'
import { Box, Text, useInput, useApp } from 'ink-terminal'
import { useBackend } from '../ipc/context.js'
import { useAppState, useAppDispatch } from '../store/app-store.js'
import { VimState } from '../vim/index.js'

export function InputPrompt() {
  const [text, setText] = useState('')
  const [cursorPos, setCursorPos] = useState(0)
  const [undoStack, setUndoStack] = useState<Array<{ text: string; cursor: number }>>([])
  const backend = useBackend()
  const { isStreaming, inputHistory, historyIndex, vimEnabled } = useAppState()
  const dispatch = useAppDispatch()
  const { exit } = useApp()
  const vimRef = useRef<VimState>(new VimState())

  // Keep vim state in sync with app state
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
  }, [text, backend, dispatch])

  useInput((input, key) => {
    if (isStreaming && key.ctrl && input === 'c') {
      backend.send({ type: 'abort_query' })
      return
    }

    if (key.ctrl && input === 'd') {
      backend.send({ type: 'quit' })
      exit()
      return
    }

    // Toggle vim mode with Escape when not in vim, or Ctrl+G
    if (key.ctrl && input === 'g') {
      vim.toggle()
      dispatch({ type: 'TOGGLE_VIM' })
      return
    }

    // If vim is enabled, route through the state machine
    if (vim.enabled) {
      const action = vim.handleKey(input, key, text, cursorPos)
      // Update the mode indicator in app state
      dispatch({ type: 'SET_VIM_MODE', mode: vim.indicator })

      switch (action.type) {
        case 'none':
          return
        case 'passthrough':
          // Fall through to default input handling below
          break
        case 'submit':
          submit()
          return
        case 'switch_mode':
          // Mode already updated on vim state; nothing else needed
          return
        case 'move_cursor':
          setCursorPos(action.pos)
          return
        case 'delete': {
          saveUndo()
          const newText = text.slice(0, action.start) + text.slice(action.end)
          setText(newText)
          setCursorPos(Math.min(action.start, newText.length))
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
          // register already set by vim state
          return
        case 'paste': {
          saveUndo()
          const newText = text.slice(0, cursorPos) + action.text + text.slice(cursorPos)
          setText(newText)
          setCursorPos(cursorPos + action.text.length)
          return
        }
        case 'insert_char': {
          saveUndo()
          const newText = text.slice(0, cursorPos) + action.char + text.slice(cursorPos)
          setText(newText)
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

    // --- Default input handling (also used for vim passthrough in insert mode) ---

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

    // Ctrl+A - beginning of line
    if (key.ctrl && input === 'a') {
      setCursorPos(0)
      return
    }

    // Ctrl+E - end of line
    if (key.ctrl && input === 'e') {
      setCursorPos(text.length)
      return
    }

    // Ctrl+U - clear line
    if (key.ctrl && input === 'u') {
      setText('')
      setCursorPos(0)
      return
    }

    // Ctrl+K - kill to end of line
    if (key.ctrl && input === 'k') {
      setText(t => t.slice(0, cursorPos))
      return
    }

    // Ctrl+W - delete word backwards
    if (key.ctrl && input === 'w') {
      const before = text.slice(0, cursorPos)
      const after = text.slice(cursorPos)
      const newBefore = before.replace(/\S+\s*$/, '')
      setText(newBefore + after)
      setCursorPos(newBefore.length)
      return
    }

    // Up/Down - history navigation
    if (key.upArrow && inputHistory.length > 0) {
      const newIdx = historyIndex === -1 ? inputHistory.length - 1 : Math.max(0, historyIndex - 1)
      dispatch({ type: 'SET_HISTORY_INDEX', index: newIdx })
      const histText = inputHistory[newIdx] || ''
      setText(histText)
      setCursorPos(histText.length)
      return
    }

    if (key.downArrow && historyIndex !== -1) {
      const newIdx = historyIndex + 1
      if (newIdx >= inputHistory.length) {
        dispatch({ type: 'SET_HISTORY_INDEX', index: -1 })
        setText('')
        setCursorPos(0)
      } else {
        dispatch({ type: 'SET_HISTORY_INDEX', index: newIdx })
        const histText = inputHistory[newIdx] || ''
        setText(histText)
        setCursorPos(histText.length)
      }
      return
    }

    // Regular character input
    if (input && !key.ctrl && !key.meta) {
      setText(t => t.slice(0, cursorPos) + input + t.slice(cursorPos))
      setCursorPos(p => p + input.length)
    }
  })

  // Render the input with cursor
  const before = text.slice(0, cursorPos)
  const cursorChar = cursorPos < text.length ? text[cursorPos] : ' '
  const after = text.slice(cursorPos + 1)

  // Prompt indicator changes based on vim mode
  const promptChar = vim.enabled
    ? (vim.mode === 'normal' ? '[N] > ' : vim.mode === 'visual' ? '[V] > ' : '[I] > ')
    : '> '

  // In visual mode, highlight the selection
  if (vim.enabled && vim.mode === 'visual') {
    const [selStart, selEnd] = cursorPos < vim.visualAnchor
      ? [cursorPos, vim.visualAnchor]
      : [vim.visualAnchor, cursorPos]

    const beforeSel = text.slice(0, selStart)
    const selected = text.slice(selStart, selEnd)
    const afterSel = text.slice(selEnd)

    return (
      <Box paddingX={1}>
        <Text color="ansi:cyanBright" bold>{promptChar}</Text>
        <Text>{beforeSel}</Text>
        <Text inverse>{selected}</Text>
        <Text>{afterSel}</Text>
      </Box>
    )
  }

  return (
    <Box paddingX={1}>
      <Text color="ansi:cyanBright" bold>{promptChar}</Text>
      <Text>{before}</Text>
      <Text inverse>{cursorChar}</Text>
      <Text>{after}</Text>
    </Box>
  )
}

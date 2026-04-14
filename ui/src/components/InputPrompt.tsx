import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useKeyboard, useRenderer } from '@opentui/react'
import type { KeyEvent, PasteEvent } from '@opentui/core'
import {
  matchesShortcut,
  shortcutLabel,
  type KeyLike,
  type ViewMode,
} from '../keybindings.js'
import { c } from '../theme.js'
import { useBackend } from '../ipc/context.js'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import { matchCommands, type CommandDef } from '../commands.js'
import { VimState } from '../vim/index.js'
import { CommandHint } from './CommandHint.js'
import {
  formatPasteSize,
  insertAtCursor,
  isPasteInput,
  isPlainTextInput,
  promptPlaceholder,
  summarizeQueuedSubmissions,
} from './input-prompt-utils.js'

const PASTE_COMPACT_CHARS = 200

function formatWorkedDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`
}

function extractInput(event: KeyEvent): string {
  const seq = event.sequence ?? ''
  if (seq.length === 1 && seq.charCodeAt(0) >= 32) return seq
  if (!event.ctrl && !event.meta && isPlainTextInput(seq)) return seq
  if (event.ctrl && (event.name?.length ?? 0) === 1) return event.name ?? ''
  if ((event.name?.length ?? 0) === 1 && !event.ctrl && !event.meta) return event.name ?? ''
  return ''
}

function toShortcutKey(event: KeyEvent): KeyLike & {
  backspace?: boolean
  delete?: boolean
  wheelUp?: boolean
  wheelDown?: boolean
} {
  const name = event.name ?? ''
  return {
    ctrl: event.ctrl ?? false,
    meta: event.meta ?? false,
    shift: event.shift ?? false,
    return: name === 'return' || name === 'enter',
    escape: name === 'escape',
    tab: name === 'tab',
    upArrow: name === 'up',
    downArrow: name === 'down',
    pageUp: name === 'pageup',
    pageDown: name === 'pagedown',
    home: name === 'home',
    end: name === 'end',
    backspace: name === 'backspace',
    delete: name === 'delete',
    wheelUp: name === 'wheel_up' || name === 'scrollup',
    wheelDown: name === 'wheel_down' || name === 'scrolldown',
  }
}

interface InputPromptProps {
  isActive?: boolean
  isReadOnly?: boolean
  onActivate?: () => void
  onStatusChange?: (status: string) => void
  viewMode: ViewMode
}

export function InputPrompt({
  isActive = true,
  isReadOnly = false,
  onActivate,
  onStatusChange,
  viewMode,
}: InputPromptProps) {
  const [text, setText] = useState('')
  const [cursorPos, setCursorPos] = useState(0)
  const [undoStack, setUndoStack] = useState<Array<{ text: string; cursor: number }>>([])
  const [isPasted, setIsPasted] = useState(false)
  const [hintIndex, setHintIndex] = useState(0)
  const [subMode, setSubMode] = useState<{ cmd: CommandDef; options: string[] } | null>(null)
  const [subIndex, setSubIndex] = useState(0)

  const backend = useBackend()
  const {
    isStreaming,
    isWaiting,
    inputHistory,
    historyIndex,
    vimEnabled,
    queuedSubmissions,
  } = useAppState()
  const dispatch = useAppDispatch()
  const renderer = useRenderer()
  const vimRef = useRef<VimState>(new VimState())
  const cursorRef = useRef(0)
  cursorRef.current = cursorPos

  const isBusy = isWaiting || isStreaming
  const isBusyRef = useRef(false)
  isBusyRef.current = isBusy

  const slashPrefix = viewMode === 'prompt' && text.startsWith('/') && !text.includes(' ') && !subMode
  const cmdPartial = slashPrefix ? text.slice(1) : ''
  const cmdMatches = useMemo(
    () => (slashPrefix ? matchCommands(cmdPartial) : []),
    [cmdPartial, slashPrefix],
  )

  useEffect(() => {
    setHintIndex(0)
  }, [cmdPartial])

  const [time, setTime] = useState(0)
  const startRef = useRef(Date.now())
  useEffect(() => {
    if (!isBusy) return
    const id = setInterval(() => setTime(Date.now() - startRef.current), 250)
    return () => clearInterval(id)
  }, [isBusy])

  const busyStartedAtRef = useRef<number | null>(null)
  const [lastWorkedMs, setLastWorkedMs] = useState(0)
  const inputActive = viewMode === 'prompt' && isActive && !isReadOnly
  const showHint = (slashPrefix || !!subMode) && !isBusy && inputActive

  useEffect(() => {
    if (isBusy) {
      if (busyStartedAtRef.current === null) busyStartedAtRef.current = time
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
  }, [cursorPos, text])

  const focusInput = useCallback(() => {
    if (isReadOnly || viewMode !== 'prompt') {
      return false
    }
    onActivate?.()
    return true
  }, [isReadOnly, onActivate, viewMode])

  const resetComposer = useCallback(() => {
    setText('')
    setCursorPos(0)
    setUndoStack([])
    setIsPasted(false)
    setSubMode(null)
    setSubIndex(0)
  }, [])

  const insertText = useCallback((value: string, options?: { pasted?: boolean }) => {
    if (!value || !focusInput()) {
      return false
    }

    saveUndo()
    const next = insertAtCursor(text, cursorPos, value)
    setText(next.text)
    setCursorPos(next.cursorPos)
    cursorRef.current = next.cursorPos
    if (options?.pasted || isPasteInput(value.length)) {
      setIsPasted(true)
    }
    if (subMode) {
      setSubMode(null)
      setSubIndex(0)
    }
    return true
  }, [cursorPos, focusInput, saveUndo, subMode, text])

  const activateInput = useCallback(() => {
    if (isReadOnly || viewMode !== 'prompt') {
      return
    }

    onActivate?.()

    if (!isBusy && text.length === 0 && historyIndex === -1 && inputHistory.length > 0) {
      const nextIndex = inputHistory.length - 1
      const latest = inputHistory[nextIndex] || ''
      dispatch({ type: 'SET_HISTORY_INDEX', index: nextIndex })
      setText(latest)
      setCursorPos(latest.length)
    }
  }, [
    dispatch,
    historyIndex,
    inputHistory,
    isBusy,
    isReadOnly,
    onActivate,
    text.length,
    viewMode,
  ])

  useEffect(() => {
    const handlePaste = (event: PasteEvent) => {
      if (viewMode !== 'prompt' || isReadOnly) {
        return
      }

      const value = new TextDecoder().decode(event.bytes)
      if (!value) {
        return
      }

      event.preventDefault()
      insertText(value, { pasted: true })
    }

    renderer._internalKeyInput.onInternal('paste', handlePaste)
    return () => {
      renderer._internalKeyInput.offInternal('paste', handlePaste)
    }
  }, [insertText, isReadOnly, renderer, viewMode])

  const sendCommand = useCallback((raw: string) => {
    if (isBusyRef.current) {
      return
    }
    const id = `user-${Date.now()}`
    dispatch({ type: 'ADD_COMMAND_MESSAGE', id, text: raw })
    dispatch({ type: 'PUSH_HISTORY', text: raw })
    backend.send({ type: 'slash_command', raw })
    resetComposer()
  }, [backend, dispatch, resetComposer])

  const queuePrompt = useCallback((raw: string) => {
    const trimmed = raw.trim()
    if (!trimmed || trimmed.startsWith('/')) {
      return false
    }

    const id = `queued-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
    dispatch({
      type: 'QUEUE_SUBMISSION',
      submission: {
        id,
        kind: 'prompt',
        text: trimmed,
        queuedAt: Date.now(),
      },
    })
    dispatch({ type: 'PUSH_HISTORY', text: trimmed })
    resetComposer()
    return true
  }, [dispatch, resetComposer])

  const submit = useCallback(() => {
    if (isBusyRef.current) {
      queuePrompt(text)
      return
    }
    const trimmed = text.trim()
    if (!trimmed) {
      return
    }

    if (trimmed.startsWith('/')) {
      sendCommand(trimmed)
      return
    }

    const id = `user-${Date.now()}`
    dispatch({ type: 'ADD_USER_MESSAGE', id, text: trimmed })
    dispatch({ type: 'PUSH_HISTORY', text: trimmed })
    backend.send({ type: 'submit_prompt', text: trimmed, id })
    resetComposer()
  }, [backend, dispatch, queuePrompt, resetComposer, sendCommand, text])

  const activateCommand = useCallback((cmd: CommandDef) => {
    if (cmd.kind === 'select' && cmd.options && cmd.options.length > 0) {
      const nextText = `/${cmd.name} `
      setText(nextText)
      setCursorPos(nextText.length)
      setSubMode({ cmd, options: cmd.options })
      setSubIndex(0)
      return
    }

    if (cmd.kind === 'input') {
      const nextText = `/${cmd.name} `
      setText(nextText)
      setCursorPos(nextText.length)
      return
    }

    sendCommand(`/${cmd.name}`)
  }, [sendCommand])

  const navigateHistoryUp = useCallback(() => {
    if (inputHistory.length === 0) return false
    const nextIndex = historyIndex === -1 ? inputHistory.length - 1 : Math.max(0, historyIndex - 1)
    dispatch({ type: 'SET_HISTORY_INDEX', index: nextIndex })
    const historyText = inputHistory[nextIndex] || ''
    setText(historyText)
    setCursorPos(historyText.length)
    return true
  }, [dispatch, historyIndex, inputHistory])

  const navigateHistoryDown = useCallback(() => {
    if (historyIndex === -1) return false
    const nextIndex = historyIndex + 1
    if (nextIndex >= inputHistory.length) {
      dispatch({ type: 'SET_HISTORY_INDEX', index: -1 })
      setText('')
      setCursorPos(0)
      return true
    }
    dispatch({ type: 'SET_HISTORY_INDEX', index: nextIndex })
    const historyText = inputHistory[nextIndex] || ''
    setText(historyText)
    setCursorPos(historyText.length)
    return true
  }, [dispatch, historyIndex, inputHistory])

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') {
      return
    }

    const input = extractInput(event)
    const key = toShortcutKey(event)
    const name = event.name

    if (matchesShortcut('app.abort', input, key, name) && isBusyRef.current) {
      backend.send({ type: 'abort_query' })
      return
    }

    if (matchesShortcut('app.quit', input, key, name)) {
      backend.send({ type: 'quit' })
      renderer.destroy()
      return
    }

    if (matchesShortcut('app.redraw', input, key, name)) {
      renderer.clearSelection()
      renderer.intermediateRender()
      return
    }

    if (matchesShortcut('app.toggleVim', input, key, name)) {
      vim.toggle()
      dispatch({ type: 'TOGGLE_VIM' })
      return
    }

    if (matchesShortcut('app.toggleTranscript', input, key, name)) {
      dispatch({ type: 'TOGGLE_VIEW_MODE' })
      return
    }

    if (viewMode === 'transcript') {
      if (matchesShortcut('transcript.exit', input, key, name)) {
        dispatch({ type: 'SET_VIEW_MODE', viewMode: 'prompt' })
      }
      return
    }

    if (!inputActive) {
      return
    }

    if (subMode) {
      const options = subMode.options

      if (matchesShortcut('list.previous', '', key, name)) {
        setSubIndex(index => (index - 1 + options.length) % options.length)
        return
      }

      if (matchesShortcut('list.next', '', key, name)) {
        setSubIndex(index => (index + 1) % options.length)
        return
      }

      if (matchesShortcut('input.confirm', input, key, name)) {
        sendCommand(`/${subMode.cmd.name} ${options[subIndex]}`)
        return
      }

      if (matchesShortcut('input.complete', input, key, name)) {
        const filled = `/${subMode.cmd.name} ${options[subIndex]}`
        setText(filled)
        setCursorPos(filled.length)
        setSubMode(null)
        return
      }

      if (matchesShortcut('input.cancel', input, key, name)) {
        const fallback = `/${subMode.cmd.name}`
        setText(fallback)
        setCursorPos(fallback.length)
        setSubMode(null)
        return
      }

      if (input && !key.ctrl && !key.meta) {
        const match = options.findIndex(option =>
          option.toLowerCase().startsWith(input.toLowerCase()),
        )
        if (match >= 0) {
          sendCommand(`/${subMode.cmd.name} ${options[match]}`)
        }
      }
      return
    }

    if (key.wheelUp && !key.ctrl && !key.meta) {
      navigateHistoryUp()
      return
    }

    if (key.wheelDown && !key.ctrl && !key.meta) {
      navigateHistoryDown()
      return
    }

    if (vim.enabled) {
      const action = vim.handleKey(input, key as any, text, cursorPos)
      dispatch({ type: 'SET_VIM_MODE', mode: vim.indicator })
      switch (action.type) {
        case 'none':
          return
        case 'passthrough':
          break
        case 'submit':
          if (!isBusyRef.current) {
            submit()
          }
          return
        case 'switch_mode':
          return
        case 'move_cursor':
          setCursorPos(action.pos)
          return
        case 'delete': {
          saveUndo()
          const nextText = text.slice(0, action.start) + text.slice(action.end)
          setText(nextText)
          setCursorPos(Math.min(action.start, nextText.length))
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
          const nextText = text.slice(0, cursorPos) + action.text + text.slice(cursorPos)
          setText(nextText)
          setCursorPos(cursorPos + action.text.length)
          return
        }
        case 'insert_char': {
          saveUndo()
          const nextText = text.slice(0, cursorPos) + action.char + text.slice(cursorPos)
          setText(nextText)
          setCursorPos(cursorPos + action.char.length)
          return
        }
        case 'undo': {
          const previous = undoStack[undoStack.length - 1]
          if (previous) {
            setUndoStack(stack => stack.slice(0, -1))
            setText(previous.text)
            setCursorPos(previous.cursor)
          }
          return
        }
      }
    }

    if (showHint && slashPrefix && cmdMatches.length > 0) {
      if (matchesShortcut('input.complete', input, key, name) || input === ' ') {
        const command = cmdMatches[hintIndex]
        if (command) activateCommand(command)
        return
      }

      if (matchesShortcut('list.previous', '', key, name) && !key.ctrl && !key.meta) {
        setHintIndex(index => (index - 1 + cmdMatches.length) % cmdMatches.length)
        return
      }

      if (matchesShortcut('list.next', '', key, name) && !key.ctrl && !key.meta) {
        setHintIndex(index => (index + 1) % cmdMatches.length)
        return
      }

      if (matchesShortcut('input.confirm', input, key, name)) {
        const command = cmdMatches[hintIndex]
        if (command && cmdPartial) {
          activateCommand(command)
        } else {
          submit()
        }
        return
      }
    }

    if (matchesShortcut('input.confirm', input, key, name)) {
      submit()
      return
    }

    if (key.backspace) {
      const pos = cursorRef.current
      if (pos > 0) {
        setText(current => current.slice(0, pos - 1) + current.slice(pos))
        cursorRef.current = pos - 1
        setCursorPos(pos - 1)
      }
      return
    }

    if (key.delete) {
      const pos = cursorRef.current
      if (pos < text.length) {
        setText(current => current.slice(0, pos) + current.slice(pos + 1))
      }
      return
    }

    if (name === 'left') {
      setCursorPos(position => Math.max(0, position - 1))
      return
    }

    if (name === 'right') {
      setCursorPos(position => Math.min(text.length, position + 1))
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
      setSubMode(null)
      return
    }

    if (key.ctrl && input === 'k') {
      setText(current => current.slice(0, cursorRef.current))
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

    if (matchesShortcut('list.previous', '', key, name) && !key.ctrl && !key.meta) {
      navigateHistoryUp()
      return
    }

    if (matchesShortcut('list.next', '', key, name) && !key.ctrl && !key.meta) {
      navigateHistoryDown()
      return
    }

    if (input && !key.ctrl && !key.meta) {
      insertText(input)
    }
  })

  const showPasteCompact = isPasted && text.length >= PASTE_COMPACT_CHARS
  const queuedPreview = queuedSubmissions.length > 0
    ? summarizeQueuedSubmissions(queuedSubmissions)
    : ''
  const before = text.slice(0, cursorPos)
  const cursorChar = cursorPos < text.length ? text[cursorPos] : ' '
  const after = cursorPos < text.length ? text.slice(cursorPos + 1) : ''
  const workedMs = isBusy && busyStartedAtRef.current !== null
    ? Math.max(0, time - busyStartedAtRef.current)
    : lastWorkedMs
  const showWorked = isBusy || lastWorkedMs > 0
  const modeTag = isStreaming ? 'reasoning' : isWaiting ? 'thinking' : ''
  const workedTag = showWorked
    ? `${modeTag ? `${modeTag} ` : ''}${formatWorkedDuration(workedMs)}`
    : ''
  const showInlineStatus = !onStatusChange && viewMode === 'prompt'

  useEffect(() => {
    onStatusChange?.(viewMode === 'prompt' && workedTag ? `* ${workedTag}` : '')
  }, [onStatusChange, viewMode, workedTag])

  return (
    <box flexDirection="column" onMouseDown={() => activateInput()}>
      <box flexDirection="row" paddingX={1}>
        <text>
          <strong>
            <span fg={inputActive ? c.accent : c.dim}>{'> '}</span>
          </strong>
        </text>
        {isReadOnly ? (
          text.length > 0 ? (
            <text fg={c.dim}>{showPasteCompact ? formatPasteSize(text) : text}</text>
          ) : (
            <text fg={c.dim}>
              Transcript mode. {shortcutLabel('app.toggleTranscript')} prompt. {shortcutLabel('transcript.exit')} exit.
            </text>
          )
        ) : showPasteCompact ? (
          <text fg={c.warningBright}>{formatPasteSize(text)}</text>
        ) : text.length === 0 ? (
          <text>
            <span fg={c.bg} bg={inputActive ? c.text : c.dim}> </span>
            <span fg="#45475A">{promptPlaceholder(isBusy)}</span>
          </text>
        ) : (
          <text fg={isBusy ? c.dim : undefined}>
            {before}
            <span fg={c.bg} bg={inputActive ? c.text : c.dim}>{cursorChar}</span>
            {after}
          </text>
        )}
        {showInlineStatus && workedTag ? (
          <text fg={c.dim}>  * {workedTag}</text>
        ) : null}
      </box>

      {showHint && !subMode && (
        <CommandHint matches={cmdMatches} selectedIndex={hintIndex} partial={cmdPartial} />
      )}
      {showHint && subMode && (
        <CommandHint
          matches={[]}
          selectedIndex={0}
          partial=""
          subOptions={subMode.options}
          subSelectedIndex={subIndex}
        />
      )}
      {viewMode === 'prompt' && queuedSubmissions.length > 0 && (
        <box paddingLeft={3}>
          <text fg={c.dim}>
            Queued {queuedSubmissions.length}: {queuedPreview}
          </text>
        </box>
      )}
    </box>
  )
}

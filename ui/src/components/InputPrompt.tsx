import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useKeyboard, useRenderer } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import {
  matchesShortcut,
  type ViewMode,
} from '../keybindings.js'
import { c } from '../theme.js'
import { useBackend } from '../ipc/context.js'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import { matchCommands, type CommandDef } from '../commands.js'
import { VimState } from '../vim/index.js'
import {
  ComposerBuffer,
  ModeIndicator,
  PromptInputFooter,
  QueuedSubmissions,
  SlashCommandHints,
  buildBusyStatus,
  deriveExternalStatus,
  extractInput,
  toShortcutKey,
  useBusyTimer,
  useComposerState,
  useComposerSubmit,
  useInputHistoryNav,
  useMaybeTruncateInput,
  usePasteHandler,
  usePromptInputPlaceholder,
} from './PromptInput/index.js'

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
  const [hintIndex, setHintIndex] = useState(0)
  const [subMode, setSubMode] = useState<{ cmd: CommandDef; options: string[] } | null>(null)
  const [subIndex, setSubIndex] = useState(0)

  const backend = useBackend()
  const {
    isStreaming,
    isWaiting,
    inputHistory,
    historyIndex,
    keybindingConfig,
    vimEnabled,
    vimMode,
    queuedSubmissions,
  } = useAppState()
  const dispatch = useAppDispatch()
  const renderer = useRenderer()
  const vimRef = useRef<VimState>(new VimState())

  const isBusy = isWaiting || isStreaming
  const isBusyRef = useRef(false)
  isBusyRef.current = isBusy

  const focusInput = useCallback(() => {
    if (isReadOnly || viewMode !== 'prompt') {
      return false
    }
    onActivate?.()
    return true
  }, [isReadOnly, onActivate, viewMode])

  const clearSubMode = useCallback(() => {
    setSubMode(null)
    setSubIndex(0)
  }, [])

  const composer = useComposerState({
    focusInput,
    onInsert: () => {
      if (subMode) clearSubMode()
    },
  })
  const {
    text,
    setText,
    cursorPos,
    setCursorPos,
    cursorRef,
    undoStack,
    setUndoStack,
    isPasted,
    setIsPasted,
    saveUndo,
    insertText,
    reset: resetBuffer,
  } = composer

  const resetComposer = useCallback(() => {
    resetBuffer()
    clearSubMode()
  }, [clearSubMode, resetBuffer])

  // Guard against pasting enough text to lag the renderer. The hook
  // swaps the middle slice for a short `[...Truncated #N +L lines...]`
  // placeholder in the buffer but keeps the full content in an internal
  // ref so `rehydrate` can splice it back on submit.
  useMaybeTruncateInput({ text, setText, setCursorPos })

  const placeholder = usePromptInputPlaceholder({
    text,
    isBusy,
    hasQueuedSubmissions: queuedSubmissions.length > 0,
  })

  const prefillInput = useCallback((nextText: string) => {
    setText(nextText)
    setCursorPos(nextText.length)
  }, [setCursorPos, setText])

  const openSubMode = useCallback(
    (cmd: CommandDef, options: string[]) => {
      setSubMode({ cmd, options })
      setSubIndex(0)
    },
    [],
  )

  const { submit, sendCommand, activateCommand } = useComposerSubmit({
    reset: resetComposer,
    openSubMode,
    prefillInput,
  })

  const slashPrefix = viewMode === 'prompt' && text.startsWith('/') && !text.includes(' ') && !subMode
  const cmdPartial = slashPrefix ? text.slice(1) : ''
  const cmdMatches = useMemo(
    () => (slashPrefix ? matchCommands(cmdPartial) : []),
    [cmdPartial, slashPrefix],
  )

  useEffect(() => {
    setHintIndex(0)
  }, [cmdPartial])

  const { time, busyStartedAtRef, lastWorkedMs } = useBusyTimer(isBusy)

  const inputActive = viewMode === 'prompt' && isActive && !isReadOnly
  const showHint = (slashPrefix || !!subMode) && !isBusy && inputActive

  const vim = vimRef.current
  if (vimEnabled && !vim.enabled) {
    vim.enable()
    dispatch({ type: 'SET_VIM_MODE', mode: vim.indicator })
  } else if (!vimEnabled && vim.enabled) {
    vim.disable()
  }

  usePasteHandler({ viewMode, isReadOnly, insertText })

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
    setCursorPos,
    setText,
    text.length,
    viewMode,
  ])

  const { navigateUp: navigateHistoryUp, navigateDown: navigateHistoryDown } = useInputHistoryNav({
    setText,
    setCursorPos,
  })

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') {
      return
    }

    const input = extractInput(event)
    const key = toShortcutKey(event)
    const name = event.name

    if (matchesShortcut('app:interrupt', input, key, name, { context: 'Global', config: keybindingConfig }) && isBusyRef.current) {
      backend.send({ type: 'abort_query' })
      return
    }

    if (matchesShortcut('app:exit', input, key, name, { context: 'Global', config: keybindingConfig })) {
      backend.send({ type: 'quit' })
      renderer.destroy()
      return
    }

    if (matchesShortcut('app:redraw', input, key, name, { context: 'Global', config: keybindingConfig })) {
      renderer.clearSelection()
      renderer.intermediateRender()
      return
    }

    if (matchesShortcut('app:toggleVim', input, key, name, { context: 'Global', config: keybindingConfig })) {
      vim.toggle()
      dispatch({ type: 'TOGGLE_VIM' })
      return
    }

    if (matchesShortcut('app:toggleTranscript', input, key, name, { context: 'Global', config: keybindingConfig })) {
      dispatch({ type: 'TOGGLE_VIEW_MODE' })
      return
    }

    if (viewMode === 'transcript') {
      if (matchesShortcut('transcript:exit', input, key, name, { context: 'Transcript', config: keybindingConfig })) {
        dispatch({ type: 'SET_VIEW_MODE', viewMode: 'prompt' })
      }
      return
    }

    if (!inputActive) {
      return
    }

    if (subMode) {
      const options = subMode.options

      if (matchesShortcut('select:previous', '', key, name, { context: 'Select', config: keybindingConfig })) {
        setSubIndex(index => (index - 1 + options.length) % options.length)
        return
      }

      if (matchesShortcut('select:next', '', key, name, { context: 'Select', config: keybindingConfig })) {
        setSubIndex(index => (index + 1) % options.length)
        return
      }

      if (matchesShortcut('select:accept', input, key, name, { context: 'Select', config: keybindingConfig })) {
        sendCommand(`/${subMode.cmd.name} ${options[subIndex]}`)
        return
      }

      if (matchesShortcut('autocomplete:accept', input, key, name, { context: 'Autocomplete', config: keybindingConfig })) {
        const filled = `/${subMode.cmd.name} ${options[subIndex]}`
        setText(filled)
        setCursorPos(filled.length)
        setSubMode(null)
        return
      }

      if (matchesShortcut('select:cancel', input, key, name, { context: 'Select', config: keybindingConfig })) {
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
            submit(text)
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
      if (
        matchesShortcut('autocomplete:accept', input, key, name, { context: 'Autocomplete', config: keybindingConfig })
        || input === ' '
      ) {
        const command = cmdMatches[hintIndex]
        if (command) activateCommand(command)
        return
      }

      if (
        matchesShortcut('select:previous', '', key, name, { context: 'Select', config: keybindingConfig })
        && !key.ctrl
        && !key.meta
      ) {
        setHintIndex(index => (index - 1 + cmdMatches.length) % cmdMatches.length)
        return
      }

      if (
        matchesShortcut('select:next', '', key, name, { context: 'Select', config: keybindingConfig })
        && !key.ctrl
        && !key.meta
      ) {
        setHintIndex(index => (index + 1) % cmdMatches.length)
        return
      }

      if (matchesShortcut('select:accept', input, key, name, { context: 'Select', config: keybindingConfig })) {
        const command = cmdMatches[hintIndex]
        if (command && cmdPartial) {
          activateCommand(command)
        } else {
          submit(text)
        }
        return
      }
    }

    if (matchesShortcut('chat:submit', input, key, name, { context: 'Chat', config: keybindingConfig })) {
      submit(text)
      return
    }

    if (matchesShortcut('chat:clearInput', input, key, name, { context: 'Chat', config: keybindingConfig })) {
      setText('')
      setCursorPos(0)
      setIsPasted(false)
      clearSubMode()
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

    if (
      matchesShortcut('history:previous', '', key, name, { context: 'Global', config: keybindingConfig })
      && !key.ctrl
      && !key.meta
    ) {
      navigateHistoryUp()
      return
    }

    if (
      matchesShortcut('history:next', '', key, name, { context: 'Global', config: keybindingConfig })
      && !key.ctrl
      && !key.meta
    ) {
      navigateHistoryDown()
      return
    }

    if (input && !key.ctrl && !key.meta) {
      insertText(input)
    }
  })

  const workedMs = isBusy && busyStartedAtRef.current !== null
    ? Math.max(0, time - busyStartedAtRef.current)
    : lastWorkedMs
  const { workedTag } = buildBusyStatus({
    isStreaming,
    isWaiting,
    isBusy,
    lastWorkedMs,
    workedMs,
  })
  const showInlineStatus = !onStatusChange && viewMode === 'prompt'

  useEffect(() => {
    onStatusChange?.(deriveExternalStatus(viewMode, workedTag))
  }, [onStatusChange, viewMode, workedTag])

  return (
    <box flexDirection="column" onMouseDown={() => activateInput()}>
      <box flexDirection="row" paddingX={1} backgroundColor={c.bg}>
        <text>
          <strong>
            <span fg={inputActive ? c.accent : c.dim} bg={c.bg}>{'> '}</span>
          </strong>
        </text>
        <ComposerBuffer
          text={text}
          cursorPos={cursorPos}
          isActive={inputActive}
          isReadOnly={isReadOnly}
          isBusy={isBusy}
          isPasted={isPasted}
          keybindingConfig={keybindingConfig}
          placeholder={placeholder}
        />
        {showInlineStatus && <ModeIndicator workedTag={workedTag} />}
      </box>

      <SlashCommandHints
        visible={showHint}
        matches={cmdMatches}
        hintIndex={hintIndex}
        partial={cmdPartial}
        subMode={subMode}
        subIndex={subIndex}
      />
      {viewMode === 'prompt' && !isReadOnly && !showHint && (
        <PromptInputFooter
          vimMode={vimEnabled ? vimMode : undefined}
          workedTag={workedTag}
          queuedCount={queuedSubmissions.length}
          isActive={inputActive}
          keybindingConfig={keybindingConfig}
        />
      )}
      {viewMode === 'prompt' && (
        <QueuedSubmissions submissions={queuedSubmissions} />
      )}
    </box>
  )
}

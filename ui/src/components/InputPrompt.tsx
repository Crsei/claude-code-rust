import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useKeyboard, useRenderer, useTerminalDimensions } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'
import { useBackend } from '../ipc/context.js'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import { VimState } from '../vim/index.js'
import { matchCommands, findCommand, type CommandDef } from '../commands.js'
import { CommandHint } from './CommandHint.js'

const PASTE_DETECT_CHARS = 100
const PASTE_COMPACT_CHARS = 200

export function isPasteInput(inputLength: number): boolean {
  return inputLength >= PASTE_DETECT_CHARS
}

export function formatPasteSize(text: string): string {
  const bytes = Buffer.byteLength(text, 'utf8')
  const kb = bytes / 1024
  return kb < 10 ? `pasted text ${Number(kb.toFixed(1))}kb` : `pasted text ${Math.round(kb)}kb`
}

function formatWorkedDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`
}

function buildSeparatorWithLabel(label: string, width: number): string {
  const safeWidth = Math.max(24, width)
  const wrappedLabel = `<${label}>`
  const rightTail = 3
  const leftLen = Math.max(1, safeWidth - wrappedLabel.length - rightTail)
  return `${'\u2500'.repeat(leftLen)}${wrappedLabel}${'\u2500'.repeat(rightTail)}`
}

function extractInput(e: KeyEvent): string {
  const seq = e.sequence ?? ''
  if (seq.length === 1 && seq.charCodeAt(0) >= 32) return seq
  if (e.ctrl && (e.name?.length ?? 0) === 1) return e.name ?? ''
  if ((e.name?.length ?? 0) === 1 && !e.ctrl && !e.meta) return e.name ?? ''
  return ''
}

function toInkKey(e: KeyEvent) {
  const name = e.name ?? ''
  return {
    upArrow: name === 'up', downArrow: name === 'down',
    leftArrow: name === 'left', rightArrow: name === 'right',
    return: name === 'return' || name === 'enter',
    escape: name === 'escape', tab: name === 'tab',
    backspace: name === 'backspace', delete: name === 'delete',
    ctrl: e.ctrl ?? false, meta: e.meta ?? false, shift: e.shift ?? false,
    wheelUp: name === 'wheel_up' || name === 'scrollup',
    wheelDown: name === 'wheel_down' || name === 'scrolldown',
  }
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
  const [hintIndex, setHintIndex] = useState(0)
  // Sub-option selection state (for select-kind commands like /effort, /model)
  const [subMode, setSubMode] = useState<{ cmd: CommandDef; options: string[] } | null>(null)
  const [subIndex, setSubIndex] = useState(0)

  const backend = useBackend()
  const { isStreaming, isWaiting, inputHistory, historyIndex, vimEnabled } = useAppState()
  const dispatch = useAppDispatch()
  const renderer = useRenderer()
  const vimRef = useRef<VimState>(new VimState())
  const isBusy = isWaiting || isStreaming
  const { width: termWidth } = useTerminalDimensions()

  // Slash command autocomplete — Phase 1: command name matching
  const slashPrefix = text.startsWith('/') && !text.includes(' ') && !subMode
  const cmdPartial = slashPrefix ? text.slice(1) : ''
  const cmdMatches = useMemo(
    () => slashPrefix ? matchCommands(cmdPartial) : [],
    [slashPrefix, cmdPartial]
  )
  useEffect(() => { setHintIndex(0) }, [cmdPartial])

  // Work timer
  const [time, setTime] = useState(0)
  const startRef = useRef(Date.now())
  useEffect(() => {
    if (!isBusy) return
    const id = setInterval(() => setTime(Date.now() - startRef.current), 250)
    return () => clearInterval(id)
  }, [isBusy])

  const busyStartedAtRef = useRef<number | null>(null)
  const [lastWorkedMs, setLastWorkedMs] = useState(0)
  const inputActive = isActive
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
  if (vimEnabled && !vim.enabled) { vim.enable(); dispatch({ type: 'SET_VIM_MODE', mode: vim.indicator }) }
  else if (!vimEnabled && vim.enabled) { vim.disable() }

  const saveUndo = useCallback(() => {
    setUndoStack(stack => [...stack.slice(-50), { text, cursor: cursorPos }])
  }, [text, cursorPos])

  const activateInput = useCallback(() => {
    onActivate?.()
    if (!isBusy && text.length === 0 && historyIndex === -1 && inputHistory.length > 0) {
      const idx = inputHistory.length - 1
      const latest = inputHistory[idx] || ''
      dispatch({ type: 'SET_HISTORY_INDEX', index: idx })
      setText(latest)
      setCursorPos(latest.length)
    }
  }, [dispatch, historyIndex, inputHistory, isBusy, onActivate, text.length])

  /** Send a slash command and clean up input state */
  const sendCommand = useCallback((raw: string) => {
    const id = `user-${Date.now()}`
    dispatch({ type: 'ADD_COMMAND_MESSAGE', id, text: raw })
    dispatch({ type: 'PUSH_HISTORY', text: raw })
    backend.send({ type: 'slash_command', raw })
    setText('')
    setCursorPos(0)
    setUndoStack([])
    setIsPasted(false)
    setSubMode(null)
    setSubIndex(0)
  }, [backend, dispatch])

  const submit = useCallback(() => {
    const trimmed = text.trim()
    if (!trimmed) return
    if (trimmed.startsWith('/')) {
      sendCommand(trimmed)
    } else {
      const id = `user-${Date.now()}`
      dispatch({ type: 'ADD_USER_MESSAGE', id, text: trimmed })
      dispatch({ type: 'PUSH_HISTORY', text: trimmed })
      backend.send({ type: 'submit_prompt', text: trimmed, id })
      setText('')
      setCursorPos(0)
      setUndoStack([])
      setIsPasted(false)
    }
  }, [backend, dispatch, text, sendCommand])

  /** Enter sub-option mode for a select-kind command, or execute immediately for toggle/action/display */
  const activateCommand = useCallback((cmd: CommandDef) => {
    if (cmd.kind === 'select' && cmd.options && cmd.options.length > 0) {
      // Show sub-option picker
      setText(`/${cmd.name} `)
      setCursorPos(cmd.name.length + 2)
      setSubMode({ cmd, options: cmd.options })
      setSubIndex(0)
      return
    }
    if (cmd.kind === 'input') {
      // Fill command name and let user type the argument
      const filled = `/${cmd.name} `
      setText(filled)
      setCursorPos(filled.length)
      return
    }
    // toggle, action, display → execute immediately
    sendCommand(`/${cmd.name}`)
  }, [sendCommand])

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

  useKeyboard((e: KeyEvent) => {
    if (e.eventType === 'release') return
    const input = extractInput(e)
    const key = toInkKey(e)

    // Global shortcuts (always active)
    if (isStreaming && key.ctrl && input === 'c') { backend.send({ type: 'abort_query' }); return }
    if (key.ctrl && input === 'd') { backend.send({ type: 'quit' }); renderer.destroy(); return }
    if (key.ctrl && input === 'g') { vim.toggle(); dispatch({ type: 'TOGGLE_VIM' }); return }
    if (!inputActive || isBusy) return

    // ── Sub-option selection mode ──
    if (subMode) {
      const opts = subMode.options
      if (e.name === 'up') { setSubIndex(i => (i - 1 + opts.length) % opts.length); return }
      if (e.name === 'down') { setSubIndex(i => (i + 1) % opts.length); return }
      if (key.return) {
        sendCommand(`/${subMode.cmd.name} ${opts[subIndex]}`)
        return
      }
      if (key.tab) {
        // Tab fills the option but doesn't submit (let user confirm with Enter)
        const filled = `/${subMode.cmd.name} ${opts[subIndex]}`
        setText(filled)
        setCursorPos(filled.length)
        setSubMode(null)
        return
      }
      if (key.escape) {
        // Cancel sub-option, go back to command name
        const back = `/${subMode.cmd.name}`
        setText(back)
        setCursorPos(back.length)
        setSubMode(null)
        return
      }
      // Typing a character selects option by first letter, then submits
      if (input && !key.ctrl && !key.meta) {
        const match = opts.findIndex(o => o.toLowerCase().startsWith(input.toLowerCase()))
        if (match >= 0) {
          sendCommand(`/${subMode.cmd.name} ${opts[match]}`)
          return
        }
      }
      return // Swallow all other keys in sub-mode
    }

    // ── Scroll wheel for history ──
    if (key.wheelUp && !key.ctrl && !key.meta) { navigateHistoryUp(); return }
    if (key.wheelDown && !key.ctrl && !key.meta) { navigateHistoryDown(); return }

    // ── Vim mode ──
    if (vim.enabled) {
      const action = vim.handleKey(input, key, text, cursorPos)
      dispatch({ type: 'SET_VIM_MODE', mode: vim.indicator })
      switch (action.type) {
        case 'none': return
        case 'passthrough': break
        case 'submit': submit(); return
        case 'switch_mode': return
        case 'move_cursor': setCursorPos(action.pos); return
        case 'delete': {
          saveUndo()
          const next = text.slice(0, action.start) + text.slice(action.end)
          setText(next); setCursorPos(Math.min(action.start, next.length)); return
        }
        case 'delete_line': saveUndo(); vim.register = text; setText(''); setCursorPos(0); return
        case 'yank': vim.register = text.slice(action.start, action.end); return
        case 'yank_line': return
        case 'paste': {
          saveUndo()
          const next = text.slice(0, cursorPos) + action.text + text.slice(cursorPos)
          setText(next); setCursorPos(cursorPos + action.text.length); return
        }
        case 'insert_char': {
          saveUndo()
          const next = text.slice(0, cursorPos) + action.char + text.slice(cursorPos)
          setText(next); setCursorPos(cursorPos + action.char.length); return
        }
        case 'undo': {
          const prev = undoStack[undoStack.length - 1]
          if (prev) { setUndoStack(s => s.slice(0, -1)); setText(prev.text); setCursorPos(prev.cursor) }
          return
        }
      }
    }

    // ── Command hint navigation (Phase 1: command name) ──
    if (showHint && slashPrefix && cmdMatches.length > 0) {
      if (key.tab || input === ' ') {
        const cmd = cmdMatches[hintIndex]
        if (cmd) activateCommand(cmd)
        return
      }
      if (e.name === 'up' && !key.ctrl && !key.meta) {
        setHintIndex(i => (i - 1 + cmdMatches.length) % cmdMatches.length)
        return
      }
      if (e.name === 'down' && !key.ctrl && !key.meta) {
        setHintIndex(i => (i + 1) % cmdMatches.length)
        return
      }
      if (key.return) {
        const cmd = cmdMatches[hintIndex]
        if (cmd && cmdPartial) {
          activateCommand(cmd)
        } else {
          submit()
        }
        return
      }
    }

    // ── Normal input handling ──
    if (key.return) { submit(); return }
    if (key.backspace) {
      if (cursorPos > 0) { setText(t => t.slice(0, cursorPos - 1) + t.slice(cursorPos)); setCursorPos(p => p - 1) }
      return
    }
    if (key.delete) {
      if (cursorPos < text.length) setText(t => t.slice(0, cursorPos) + t.slice(cursorPos + 1))
      return
    }
    if (e.name === 'left') { setCursorPos(p => Math.max(0, p - 1)); return }
    if (e.name === 'right') { setCursorPos(p => Math.min(text.length, p + 1)); return }
    if (key.ctrl && input === 'a') { setCursorPos(0); return }
    if (key.ctrl && input === 'e') { setCursorPos(text.length); return }
    if (key.ctrl && input === 'u') { setText(''); setCursorPos(0); setIsPasted(false); setSubMode(null); return }
    if (key.ctrl && input === 'k') { setText(t => t.slice(0, cursorPos)); return }
    if (key.ctrl && input === 'w') {
      const before = text.slice(0, cursorPos)
      const after = text.slice(cursorPos)
      const nextBefore = before.replace(/\S+\s*$/, '')
      setText(nextBefore + after); setCursorPos(nextBefore.length); return
    }
    if (e.name === 'up' && !key.ctrl && !key.meta) { navigateHistoryUp(); return }
    if (e.name === 'down' && !key.ctrl && !key.meta) { navigateHistoryDown(); return }

    if (input && !key.ctrl && !key.meta) {
      if (isPasteInput(input.length)) setIsPasted(true)
      setText(t => t.slice(0, cursorPos) + input + t.slice(cursorPos))
      setCursorPos(p => p + input.length)
    }
  })

  const showPasteCompact = isPasted && text.length >= PASTE_COMPACT_CHARS
  const before = text.slice(0, cursorPos)
  const cursorChar = cursorPos < text.length ? text[cursorPos] : ' '
  const after = cursorPos < text.length ? text.slice(cursorPos + 1) : ''
  const workedMs = isBusy && busyStartedAtRef.current !== null
    ? Math.max(0, time - busyStartedAtRef.current) : lastWorkedMs
  const showWorked = isBusy || lastWorkedMs > 0

  // Status tag: "reasoning 3s" / "thinking 2s" / "3s"
  const modeTag = isStreaming ? 'reasoning' : isWaiting ? 'thinking' : ''
  const workedTag = showWorked
    ? `${modeTag ? modeTag + ' ' : ''}${formatWorkedDuration(workedMs)}`
    : ''

  return (
    <box flexDirection="column" onMouseDown={() => activateInput()}>
      {/* ── Main input line: ❯ text█ ── */}
      <box flexDirection="row" paddingX={1}>
        <text><strong><span fg={inputActive ? c.accent : c.dim}>{'\u276f '}</span></strong></text>
        {isBusy ? (
          <text fg={c.dim}>{showPasteCompact ? formatPasteSize(text) : text}</text>
        ) : showPasteCompact ? (
          <text fg={c.warningBright}>{formatPasteSize(text)}</text>
        ) : text.length === 0 ? (
          <text>
            <span fg={c.bg} bg={inputActive ? c.text : c.dim}> </span>
            <span fg="#45475A"> Type a message or /command...</span>
          </text>
        ) : (
          <text>
            {before}
            <span fg={c.bg} bg={inputActive ? c.text : c.dim}>{cursorChar}</span>
            {after}
          </text>
        )}
        {/* Inline status at the end */}
        {workedTag ? (
          <text fg={c.dim}>  {'\u273b'} {workedTag}</text>
        ) : null}
      </box>

      {/* ── Command hints (below input line) ── */}
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
    </box>
  )
}

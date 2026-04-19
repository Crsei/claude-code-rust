import { useCallback, useEffect, useRef, useState } from 'react'
import type { PasteEvent } from '@opentui/core'
import { useRenderer } from '@opentui/react'
import { useAppDispatch, useAppState } from '../store/app-store.js'
import type { ViewMode } from '../keybindings.js'
import { insertAtCursor, isPasteInput } from './input-prompt-utils.js'

/**
 * Composer state: text buffer, cursor, undo stack, paste indicator.
 *
 * Exposes live refs for the text/cursor so the keyboard handler can read
 * the latest values without going through a re-render. `insertText` /
 * `reset` take care of resetting the paste indicator and any sub-mode the
 * caller opts into via `onInsert`.
 */
export interface ComposerState {
  text: string
  setText: (value: string | ((prev: string) => string)) => void
  cursorPos: number
  setCursorPos: (value: number | ((prev: number) => number)) => void
  textRef: React.MutableRefObject<string>
  cursorRef: React.MutableRefObject<number>
  undoStack: Array<{ text: string; cursor: number }>
  setUndoStack: React.Dispatch<React.SetStateAction<Array<{ text: string; cursor: number }>>>
  isPasted: boolean
  setIsPasted: (value: boolean) => void
  saveUndo: () => void
  insertText: (value: string, options?: { pasted?: boolean }) => boolean
  reset: () => void
}

export function useComposerState(params: {
  focusInput: () => boolean
  onInsert?: () => void
}): ComposerState {
  const { focusInput, onInsert } = params
  const [text, setText] = useState('')
  const [cursorPos, setCursorPos] = useState(0)
  const [undoStack, setUndoStack] = useState<Array<{ text: string; cursor: number }>>([])
  const [isPasted, setIsPasted] = useState(false)

  const textRef = useRef('')
  textRef.current = text
  const cursorRef = useRef(0)
  cursorRef.current = cursorPos

  const saveUndo = useCallback(() => {
    setUndoStack(stack => [...stack.slice(-50), {
      text: textRef.current,
      cursor: cursorRef.current,
    }])
  }, [])

  const insertText = useCallback((value: string, options?: { pasted?: boolean }) => {
    if (!value || !focusInput()) {
      return false
    }
    saveUndo()
    const next = insertAtCursor(textRef.current, cursorRef.current, value)
    textRef.current = next.text
    cursorRef.current = next.cursorPos
    setText(next.text)
    setCursorPos(next.cursorPos)
    if (options?.pasted || isPasteInput(value.length)) {
      setIsPasted(true)
    }
    onInsert?.()
    return true
  }, [focusInput, onInsert, saveUndo])

  const reset = useCallback(() => {
    setText('')
    setCursorPos(0)
    setUndoStack([])
    setIsPasted(false)
  }, [])

  return {
    text,
    setText,
    cursorPos,
    setCursorPos,
    textRef,
    cursorRef,
    undoStack,
    setUndoStack,
    isPasted,
    setIsPasted,
    saveUndo,
    insertText,
    reset,
  }
}

/**
 * Busy timer: tracks elapsed time for the current busy run, emitting the
 * final elapsed duration into `lastWorkedMs` when the run ends.
 */
export function useBusyTimer(isBusy: boolean) {
  const [time, setTime] = useState(0)
  const startRef = useRef(Date.now())
  useEffect(() => {
    if (!isBusy) return
    const id = setInterval(() => setTime(Date.now() - startRef.current), 250)
    return () => clearInterval(id)
  }, [isBusy])

  const busyStartedAtRef = useRef<number | null>(null)
  const [lastWorkedMs, setLastWorkedMs] = useState(0)

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

  return { time, busyStartedAtRef, lastWorkedMs }
}

/**
 * Paste handler: subscribes to the renderer's internal paste event while
 * the prompt is the active input. The caller supplies `insertText` so the
 * paste is routed through the same composer path as typed input.
 */
export function usePasteHandler(params: {
  viewMode: ViewMode
  isReadOnly: boolean
  insertText: (value: string, options?: { pasted?: boolean }) => boolean
}) {
  const { viewMode, isReadOnly, insertText } = params
  const renderer = useRenderer()

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
}

/**
 * Input history navigation: up/down cycling through `state.inputHistory`
 * with the store's `historyIndex` as the cursor.
 */
export function useInputHistoryNav(params: {
  setText: (value: string) => void
  setCursorPos: (value: number) => void
}) {
  const { setText, setCursorPos } = params
  const { inputHistory, historyIndex } = useAppState()
  const dispatch = useAppDispatch()

  const navigateUp = useCallback(() => {
    if (inputHistory.length === 0) return false
    const nextIndex = historyIndex === -1 ? inputHistory.length - 1 : Math.max(0, historyIndex - 1)
    dispatch({ type: 'SET_HISTORY_INDEX', index: nextIndex })
    const historyText = inputHistory[nextIndex] || ''
    setText(historyText)
    setCursorPos(historyText.length)
    return true
  }, [dispatch, historyIndex, inputHistory, setCursorPos, setText])

  const navigateDown = useCallback(() => {
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
  }, [dispatch, historyIndex, inputHistory, setCursorPos, setText])

  return { navigateUp, navigateDown }
}

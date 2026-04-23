import { useCallback, useRef } from 'react'
import type { CommandDef } from '../../commands.js'
import { useBackend } from '../../ipc/context.js'
import { useAppDispatch, useAppState } from '../../store/app-store.js'

/**
 * Submission + queue + slash-command dispatch hook for the composer.
 *
 * Centralizes the code that decides which IPC message to send (and
 * which store action to dispatch) when the user presses Enter. Kept
 * out of `InputPrompt.tsx` so the keyboard callback stays focused on
 * key-to-intent mapping and so submission can be unit-tested via the
 * store dispatch + backend spy.
 *
 * Contracts preserved from the previous monolithic composer:
 * - Idle user text → `submit_prompt` + `ADD_USER_MESSAGE` + history push
 * - Idle user text starting with `/` → `slash_command` + `ADD_COMMAND_MESSAGE`
 * - Idle user text during an ask-user question → `question_response` + dismiss
 * - Busy prompt submission (and no pending question) → queue under the store
 * - `activateCommand` → starts select sub-modes or pre-fills input-kind commands
 *
 * The caller owns the composer buffer; this hook only asks for a
 * `reset` callback (clears buffer + sub-mode) and the current
 * sub-mode opener.
 */

export interface UseComposerSubmitParams {
  reset: () => void
  openSubMode: (cmd: CommandDef, options: string[]) => void
  prefillInput: (text: string) => void
}

export interface ComposerSubmit {
  /** Submit the given text — picks the right IPC message based on
   *  current busy state, pending question, and leading slash. */
  submit: (text: string) => void
  /** Send a pre-formed `/name [arg]` slash command. Used by sub-mode
   *  resolution after the user picks an option. */
  sendCommand: (raw: string) => void
  /** Activate a selected command hint — either open the sub-mode
   *  selector, pre-fill the input, or send the command directly. */
  activateCommand: (cmd: CommandDef) => void
}

export function useComposerSubmit({
  reset,
  openSubMode,
  prefillInput,
}: UseComposerSubmitParams): ComposerSubmit {
  const backend = useBackend()
  const dispatch = useAppDispatch()
  const { isStreaming, isWaiting, pendingQuestion } = useAppState()

  // Track the current busy flag via ref so callbacks invoked from a
  // keyboard handler see the latest value without needing to be
  // re-created on every busy-flag transition.
  const isBusyRef = useRef(false)
  isBusyRef.current = isStreaming || isWaiting

  const pendingQuestionRef = useRef(pendingQuestion)
  pendingQuestionRef.current = pendingQuestion

  const sendCommand = useCallback((raw: string) => {
    if (isBusyRef.current) {
      return
    }
    // Client-side-only commands that open UI dialogs without contacting
    // the backend. Each one has a corresponding store action; add new
    // entries here when introducing another UI-driven modal.
    const trimmed = raw.trim()
    const head = trimmed.split(/\s+/)[0] ?? ''
    if (head === '/agents-ui' || head === '/au') {
      dispatch({ type: 'PUSH_HISTORY', text: raw })
      dispatch({ type: 'AGENT_SETTINGS_OPEN' })
      reset()
      return
    }
    // `/mcp` with no subcommand opens the dialog; `/mcp list` etc still fall
    // through to the backend slash-command handler for the CLI text output.
    if (head === '/mcp' && trimmed === '/mcp') {
      dispatch({ type: 'PUSH_HISTORY', text: raw })
      dispatch({ type: 'MCP_SETTINGS_OPEN' })
      reset()
      return
    }
    const id = `user-${Date.now()}`
    dispatch({ type: 'ADD_COMMAND_MESSAGE', id, text: raw })
    dispatch({ type: 'PUSH_HISTORY', text: raw })
    backend.send({ type: 'slash_command', raw })
    reset()
  }, [backend, dispatch, reset])

  const queuePrompt = useCallback((raw: string) => {
    const trimmed = raw.trim()
    if (!trimmed || trimmed.startsWith('/')) return false
    const id = `queued-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
    dispatch({
      type: 'QUEUE_SUBMISSION',
      submission: { id, kind: 'prompt', text: trimmed, queuedAt: Date.now() },
    })
    dispatch({ type: 'PUSH_HISTORY', text: trimmed })
    reset()
    return true
  }, [dispatch, reset])

  const submit = useCallback((text: string) => {
    const pending = pendingQuestionRef.current
    if (isBusyRef.current && !pending) {
      queuePrompt(text)
      return
    }
    const trimmed = text.trim()
    if (!trimmed) return

    if (pending) {
      dispatch({ type: 'ADD_USER_MESSAGE', id: `answer-${pending.id}`, text: trimmed })
      dispatch({ type: 'PUSH_HISTORY', text: trimmed })
      backend.send({ type: 'question_response', id: pending.id, text: trimmed })
      dispatch({ type: 'QUESTION_DISMISS' })
      reset()
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
    reset()
  }, [backend, dispatch, queuePrompt, reset, sendCommand])

  const activateCommand = useCallback((cmd: CommandDef) => {
    if (cmd.kind === 'select' && cmd.options && cmd.options.length > 0) {
      const nextText = `/${cmd.name} `
      prefillInput(nextText)
      openSubMode(cmd, cmd.options)
      return
    }
    if (cmd.kind === 'input') {
      prefillInput(`/${cmd.name} `)
      return
    }
    sendCommand(`/${cmd.name}`)
  }, [openSubMode, prefillInput, sendCommand])

  return { submit, sendCommand, activateCommand }
}

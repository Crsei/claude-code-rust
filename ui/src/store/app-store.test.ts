import { describe, expect, test } from 'bun:test'
import { appReducer, initialState } from './app-store.js'

describe('appReducer streaming lifecycle', () => {
  test('stream end clears busy flags and partial text', () => {
    const waiting = appReducer(initialState, { type: 'ADD_USER_MESSAGE', id: 'user-1', text: 'hello' })
    const streaming = appReducer(waiting, { type: 'STREAM_START', messageId: 'stream-1' })
    const partial = appReducer(streaming, { type: 'STREAM_DELTA', text: 'partial answer' })
    const ended = appReducer(partial, { type: 'STREAM_END' })

    expect(ended.isWaiting).toBe(false)
    expect(ended.isStreaming).toBe(false)
    expect(ended.streamingText).toBe('')
    expect(ended.streamingMessageId).toBeNull()
  })

  test('assistant completion remains clean even if stream_end arrives after the final message', () => {
    const waiting = appReducer(initialState, { type: 'ADD_USER_MESSAGE', id: 'user-1', text: 'hello' })
    const streaming = appReducer(waiting, { type: 'STREAM_START', messageId: 'stream-1' })
    const partial = appReducer(streaming, { type: 'STREAM_DELTA', text: 'partial answer' })
    const completed = appReducer(partial, {
      type: 'ASSISTANT_MESSAGE',
      id: 'assistant-1',
      content: 'final answer',
      costUsd: 0.01,
      contentBlocks: [{ type: 'text', text: 'final answer' }],
    })
    const settled = appReducer(completed, { type: 'STREAM_END' })

    expect(settled.isWaiting).toBe(false)
    expect(settled.isStreaming).toBe(false)
    expect(settled.streamingText).toBe('')
    expect(settled.messages.at(-1)?.role).toBe('assistant')
    expect(settled.messages.at(-1)?.content).toBe('final answer')
    expect(settled.messages.at(-1)?.contentBlocks).toEqual([{ type: 'text', text: 'final answer' }])
  })

  test('error clears streaming state before showing the system message', () => {
    const waiting = appReducer(initialState, { type: 'ADD_USER_MESSAGE', id: 'user-1', text: 'hello' })
    const streaming = appReducer(waiting, { type: 'STREAM_START', messageId: 'stream-1' })
    const partial = appReducer(streaming, { type: 'STREAM_DELTA', text: 'partial answer' })
    const errored = appReducer(partial, { type: 'ERROR', message: 'request failed' })

    expect(errored.isWaiting).toBe(false)
    expect(errored.isStreaming).toBe(false)
    expect(errored.streamingText).toBe('')
    expect(errored.streamingMessageId).toBeNull()
    expect(errored.messages.at(-1)?.role).toBe('system')
    expect(errored.messages.at(-1)?.level).toBe('error')
  })
})

describe('appReducer view mode', () => {
  test('ready hydrates editor mode, view mode, and keybinding config', () => {
    const ready = appReducer(initialState, {
      type: 'READY',
      model: 'gpt-test',
      sessionId: 'session-1',
      cwd: '/tmp',
      editorMode: 'vim',
      viewMode: 'transcript',
      keybindings: {
        bindings: [
          {
            context: 'Chat',
            bindings: { enter: 'chat:newline' },
          },
        ],
      },
    })

    expect(ready.editorMode).toBe('vim')
    expect(ready.vimEnabled).toBe(true)
    expect(ready.vimMode).toBe('NORMAL')
    expect(ready.viewMode).toBe('transcript')
    expect(ready.keybindingConfig?.bindings?.[0]?.context).toBe('Chat')
  })

  test('toggle transcript mode flips between prompt and transcript', () => {
    const transcript = appReducer(initialState, { type: 'TOGGLE_VIEW_MODE' })
    const prompt = appReducer(transcript, { type: 'TOGGLE_VIEW_MODE' })

    expect(transcript.viewMode).toBe('transcript')
    expect(prompt.viewMode).toBe('prompt')
  })
})

describe('appReducer queued submissions', () => {
  test('toggle vim keeps editor mode and mode label in sync', () => {
    const enabled = appReducer(initialState, { type: 'TOGGLE_VIM' })
    const disabled = appReducer(enabled, { type: 'TOGGLE_VIM' })

    expect(enabled.editorMode).toBe('vim')
    expect(enabled.vimEnabled).toBe(true)
    expect(enabled.vimMode).toBe('NORMAL')
    expect(disabled.editorMode).toBe('normal')
    expect(disabled.vimEnabled).toBe(false)
    expect(disabled.vimMode).toBe('')
  })

  test('queues and dequeues prompt submissions in FIFO order', () => {
    const first = appReducer(initialState, {
      type: 'QUEUE_SUBMISSION',
      submission: {
        id: 'queued-1',
        kind: 'prompt',
        text: 'first',
        queuedAt: 1,
      },
    })
    const second = appReducer(first, {
      type: 'QUEUE_SUBMISSION',
      submission: {
        id: 'queued-2',
        kind: 'prompt',
        text: 'second',
        queuedAt: 2,
      },
    })
    const drained = appReducer(second, { type: 'DEQUEUE_SUBMISSION' })

    expect(second.queuedSubmissions.map(item => item.text)).toEqual(['first', 'second'])
    expect(drained.queuedSubmissions.map(item => item.text)).toEqual(['second'])
  })
})

describe('appReducer LSP live editor state', () => {
  test('stores completion results by request id', () => {
    const next = appReducer(initialState, {
      type: 'LSP_COMPLETION_RESULTS',
      requestId: 'completion-1',
      uri: 'file:///src/main.rs',
      items: [{ label: 'println!', kind: 'Function' }],
    })

    expect(next.lspCompletions.byRequestId['completion-1']?.uri).toBe('file:///src/main.rs')
    expect(next.lspCompletions.byRequestId['completion-1']?.items[0]?.label).toBe('println!')
    expect(next.lspCompletions.lastError).toBeNull()
  })

  test('stores and clears live diagnostics by uri', () => {
    const published = appReducer(initialState, {
      type: 'LSP_DIAGNOSTICS_PUBLISHED',
      uri: 'file:///src/main.rs',
      diagnostics: [
        {
          range: { start_line: 2, start_character: 4, end_line: 2, end_character: 8 },
          severity: 'error',
          message: 'expected expression',
        },
      ],
    })
    const cleared = appReducer(published, {
      type: 'LSP_DIAGNOSTICS_PUBLISHED',
      uri: 'file:///src/main.rs',
      diagnostics: [],
    })

    expect(published.diagnostics.byUri['file:///src/main.rs']).toHaveLength(1)
    expect(cleared.diagnostics.byUri['file:///src/main.rs']).toBeUndefined()
  })
})

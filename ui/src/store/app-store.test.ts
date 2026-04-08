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
    })
    const settled = appReducer(completed, { type: 'STREAM_END' })

    expect(settled.isWaiting).toBe(false)
    expect(settled.isStreaming).toBe(false)
    expect(settled.streamingText).toBe('')
    expect(settled.messages.at(-1)?.role).toBe('assistant')
    expect(settled.messages.at(-1)?.content).toBe('final answer')
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

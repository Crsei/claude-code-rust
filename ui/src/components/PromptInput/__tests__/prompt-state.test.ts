import { describe, expect, test } from 'bun:test'
import {
  buildBusyStatus,
  deriveExternalStatus,
  shouldRenderPasteCompact,
  splitBufferAtCursor,
  PASTE_COMPACT_CHARS,
} from '../prompt-state.js'

describe('splitBufferAtCursor', () => {
  test('splits around an interior cursor', () => {
    expect(splitBufferAtCursor('hello', 2)).toEqual({
      before: 'he',
      cursorChar: 'l',
      after: 'lo',
    })
  })

  test('emits a space sentinel when the cursor sits past the last char', () => {
    expect(splitBufferAtCursor('hi', 2)).toEqual({
      before: 'hi',
      cursorChar: ' ',
      after: '',
    })
  })

  test('clamps negative cursors to zero', () => {
    expect(splitBufferAtCursor('abc', -5)).toEqual({
      before: '',
      cursorChar: 'a',
      after: 'bc',
    })
  })

  test('clamps cursors past end to length', () => {
    expect(splitBufferAtCursor('abc', 99)).toEqual({
      before: 'abc',
      cursorChar: ' ',
      after: '',
    })
  })
})

describe('shouldRenderPasteCompact', () => {
  test('requires both the paste flag and a long enough buffer', () => {
    expect(shouldRenderPasteCompact(true, PASTE_COMPACT_CHARS)).toBe(true)
    expect(shouldRenderPasteCompact(true, PASTE_COMPACT_CHARS - 1)).toBe(false)
    expect(shouldRenderPasteCompact(false, PASTE_COMPACT_CHARS + 100)).toBe(false)
  })
})

describe('buildBusyStatus', () => {
  test('returns empty tags when idle and there is no prior run', () => {
    const status = buildBusyStatus({
      isStreaming: false,
      isWaiting: false,
      isBusy: false,
      lastWorkedMs: 0,
      workedMs: 0,
    })
    expect(status).toEqual({ modeTag: '', workedTag: '' })
  })

  test('prefixes duration with reasoning while streaming', () => {
    const status = buildBusyStatus({
      isStreaming: true,
      isWaiting: false,
      isBusy: true,
      lastWorkedMs: 0,
      workedMs: 12_000,
    })
    expect(status.modeTag).toBe('reasoning')
    expect(status.workedTag).toBe('reasoning 12s')
  })

  test('prefixes duration with thinking while waiting', () => {
    const status = buildBusyStatus({
      isStreaming: false,
      isWaiting: true,
      isBusy: true,
      lastWorkedMs: 0,
      workedMs: 61_000,
    })
    expect(status.modeTag).toBe('thinking')
    expect(status.workedTag).toBe('thinking 1m 1s')
  })

  test('shows bare duration after a completed run', () => {
    const status = buildBusyStatus({
      isStreaming: false,
      isWaiting: false,
      isBusy: false,
      lastWorkedMs: 3_000,
      workedMs: 3_000,
    })
    expect(status.modeTag).toBe('')
    expect(status.workedTag).toBe('3s')
  })
})

describe('deriveExternalStatus', () => {
  test('returns empty string in transcript mode', () => {
    expect(deriveExternalStatus('transcript', 'reasoning 3s')).toBe('')
  })

  test('returns empty string when tag is empty', () => {
    expect(deriveExternalStatus('prompt', '')).toBe('')
  })

  test('prefixes the tag with a star in prompt mode', () => {
    expect(deriveExternalStatus('prompt', 'reasoning 3s')).toBe('* reasoning 3s')
  })
})

import { describe, expect, test } from 'bun:test'
import {
  formatPasteSize,
  isPasteInput,
  promptPlaceholder,
  summarizeQueuedSubmissions,
} from '../InputPrompt.js'

describe('isPasteInput', () => {
  test('single char is not paste', () => {
    expect(isPasteInput(1)).toBe(false)
  })

  test('short multi-char input is not paste', () => {
    expect(isPasteInput(10)).toBe(false)
    expect(isPasteInput(50)).toBe(false)
    expect(isPasteInput(99)).toBe(false)
  })

  test('100+ chars detected as paste', () => {
    expect(isPasteInput(100)).toBe(true)
    expect(isPasteInput(200)).toBe(true)
    expect(isPasteInput(5000)).toBe(true)
  })
})

describe('formatPasteSize', () => {
  test('small text shows fractional kb', () => {
    const text = 'a'.repeat(100)
    expect(formatPasteSize(text)).toBe('pasted text 0.1kb')
  })

  test('sub-1kb text shows one decimal', () => {
    const text = 'a'.repeat(512)
    expect(formatPasteSize(text)).toBe('pasted text 0.5kb')
  })

  test('1kb text', () => {
    const text = 'a'.repeat(1024)
    expect(formatPasteSize(text)).toBe('pasted text 1kb')
  })

  test('several kb text shows one decimal', () => {
    const text = 'a'.repeat(2560)
    expect(formatPasteSize(text)).toBe('pasted text 2.5kb')
  })

  test('10kb+ rounds to integer', () => {
    const text = 'a'.repeat(10240)
    expect(formatPasteSize(text)).toBe('pasted text 10kb')
  })

  test('large text rounds to integer', () => {
    const text = 'a'.repeat(51200)
    expect(formatPasteSize(text)).toBe('pasted text 50kb')
  })

  test('multibyte unicode counted correctly', () => {
    const text = '\u4f60'.repeat(341)
    expect(formatPasteSize(text)).toBe('pasted text 1kb')
  })
})

describe('promptPlaceholder', () => {
  test('shows drafting hint while busy', () => {
    expect(promptPlaceholder(true)).toContain('draft the next message')
  })

  test('shows normal composer hint when idle', () => {
    expect(promptPlaceholder(false)).toContain('Type a message')
  })
})

describe('summarizeQueuedSubmissions', () => {
  test('joins a short queue into a single preview line', () => {
    expect(summarizeQueuedSubmissions([
      { text: 'first follow-up' },
      { text: 'second follow-up' },
    ])).toContain('first follow-up')
  })

  test('shows overflow count for long queues', () => {
    expect(summarizeQueuedSubmissions([
      { text: 'one' },
      { text: 'two' },
      { text: 'three' },
    ])).toContain('+1 more')
  })
})

import { describe, expect, test } from 'bun:test'
import { isPasteInput, formatPasteSize } from '../InputPrompt.js'

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
    const text = 'a'.repeat(100) // 100 bytes
    expect(formatPasteSize(text)).toBe('pasted text 0.1kb')
  })

  test('sub-1kb text shows one decimal', () => {
    const text = 'a'.repeat(512) // 512 bytes = 0.5kb
    expect(formatPasteSize(text)).toBe('pasted text 0.5kb')
  })

  test('1kb text', () => {
    const text = 'a'.repeat(1024) // 1024 bytes = 1kb
    expect(formatPasteSize(text)).toBe('pasted text 1kb')
  })

  test('several kb text shows one decimal', () => {
    const text = 'a'.repeat(2560) // 2560 bytes = 2.5kb
    expect(formatPasteSize(text)).toBe('pasted text 2.5kb')
  })

  test('10kb+ rounds to integer', () => {
    const text = 'a'.repeat(10240) // 10240 bytes = 10kb
    expect(formatPasteSize(text)).toBe('pasted text 10kb')
  })

  test('large text rounds to integer', () => {
    const text = 'a'.repeat(51200) // 50kb
    expect(formatPasteSize(text)).toBe('pasted text 50kb')
  })

  test('multibyte unicode counted correctly', () => {
    // Chinese chars are 3 bytes each in UTF-8
    const text = '\u4f60'.repeat(341) // 341 * 3 = 1023 bytes ≈ 1kb
    const result = formatPasteSize(text)
    expect(result).toBe('pasted text 1kb')
  })
})

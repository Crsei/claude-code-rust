import { describe, expect, test } from 'bun:test'
import { stringWidth, truncateToWidth } from '../string-width.js'

describe('stringWidth', () => {
  test('counts ASCII characters as width 1', () => {
    expect(stringWidth('hello')).toBe(5)
    expect(stringWidth('')).toBe(0)
  })

  test('counts CJK characters as width 2', () => {
    expect(stringWidth('日本語')).toBe(6)
    expect(stringWidth('한글')).toBe(4)
  })

  test('ignores C0 and C1 control characters', () => {
    expect(stringWidth('a\x00b')).toBe(2)
    expect(stringWidth('a\x1bb')).toBe(2) // ESC does not contribute width
    expect(stringWidth('a\x7fb')).toBe(2) // DEL does not contribute width
  })

  test('handles astral plane CJK ideographs as width 2', () => {
    // U+20000 CJK Extension B
    expect(stringWidth('\u{20000}')).toBe(2)
  })
})

describe('truncateToWidth', () => {
  test('returns the original string when it already fits', () => {
    expect(truncateToWidth('hello', 10)).toBe('hello')
  })

  test('truncates and appends an ellipsis', () => {
    expect(truncateToWidth('hello world', 6)).toBe('hello\u2026')
  })

  test('returns an empty string for non-positive budgets', () => {
    expect(truncateToWidth('hello', 0)).toBe('')
    expect(truncateToWidth('hello', -1)).toBe('')
  })

  test('respects CJK widths when fitting', () => {
    // "日本" (width 4) fits into width 5 budget — no truncation
    expect(truncateToWidth('日本', 5)).toBe('日本')
    // Force truncation: budget 3 => one wide char (width 2) + ellipsis (width 1)
    expect(truncateToWidth('日本', 3)).toBe('日\u2026')
  })
})

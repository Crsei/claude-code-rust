import { describe, expect, test } from 'bun:test'
import { formatCell } from '../ServerListEditor.js'

describe('formatCell', () => {
  test('returns text unchanged when width is null', () => {
    expect(formatCell('hello', null)).toBe('hello')
  })

  test('right-pads short text with spaces to the requested width', () => {
    expect(formatCell('abc', 6)).toBe('abc   ')
  })

  test('truncates text that is longer than the column width', () => {
    expect(formatCell('longtext', 4)).toBe('long')
  })

  test('returns exact-fit text unchanged at the boundary', () => {
    expect(formatCell('abcd', 4)).toBe('abcd')
  })

  test('handles empty input by padding to the width', () => {
    expect(formatCell('', 3)).toBe('   ')
  })
})

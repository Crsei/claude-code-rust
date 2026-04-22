import { describe, expect, test } from 'bun:test'
import { fileUrl, osc8Link } from '../FilePathLink.js'

describe('fileUrl', () => {
  test('returns a file:// href for an absolute path', () => {
    const href = fileUrl('/tmp/example.txt')
    expect(href.startsWith('file://')).toBe(true)
    expect(href.endsWith('/example.txt')).toBe(true)
  })

  test('escapes spaces in the path', () => {
    const href = fileUrl('/tmp/a b/c.txt')
    expect(href).toContain('a%20b')
  })
})

describe('osc8Link', () => {
  test('wraps text in OSC 8 hyperlink escape sequences', () => {
    const result = osc8Link('file:///tmp/example.txt', 'example.txt')
    expect(result).toBe('\x1b]8;;file:///tmp/example.txt\x1b\\example.txt\x1b]8;;\x1b\\')
  })

  test('round-trips through plain text terminals (stripping escapes yields the label)', () => {
    const result = osc8Link('file:///tmp/f.txt', 'label')
    const plain = result.replace(/\x1b\]8;;[^\x1b]*\x1b\\/g, '')
    expect(plain).toBe('label')
  })
})

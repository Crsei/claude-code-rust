import { describe, expect, test } from 'bun:test'
import { systemLevelFromRaw } from '../messages.js'

describe('systemLevelFromRaw', () => {
  test('falls back to info when level is missing', () => {
    expect(systemLevelFromRaw(undefined)).toBe('info')
    expect(systemLevelFromRaw('')).toBe('info')
  })

  test('normalizes known levels through lowercasing', () => {
    expect(systemLevelFromRaw('WARNING')).toBe('warning')
    expect(systemLevelFromRaw('Error')).toBe('error')
    expect(systemLevelFromRaw('success')).toBe('success')
    expect(systemLevelFromRaw('debug')).toBe('debug')
    expect(systemLevelFromRaw('info')).toBe('info')
  })

  test('falls back to info for unknown levels (including ask-user question)', () => {
    expect(systemLevelFromRaw('question')).toBe('info')
    expect(systemLevelFromRaw('pink')).toBe('info')
  })
})

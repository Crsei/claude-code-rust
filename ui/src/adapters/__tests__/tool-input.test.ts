import { describe, expect, test } from 'bun:test'
import { describeToolInput, summarizeToolInput } from '../tool-input.js'

describe('describeToolInput', () => {
  test('prefers command over file_path when both are present', () => {
    expect(describeToolInput({ command: 'ls -la', file_path: '/tmp' })).toBe(
      'ls -la',
    )
  })

  test('renders pattern + path combos', () => {
    expect(describeToolInput({ pattern: '*.ts', path: '/tmp' })).toBe(
      '"*.ts" in /tmp',
    )
  })

  test('returns the raw string when the input is already a string', () => {
    expect(describeToolInput('already a string')).toBe('already a string')
  })

  test('falls back to JSON when no known field is present', () => {
    expect(describeToolInput({ custom: 'value' })).toBe('{"custom":"value"}')
  })

  test('treats null/undefined as empty inline string', () => {
    expect(describeToolInput(null)).toBe('')
    expect(describeToolInput(undefined)).toBe('')
  })
})

describe('summarizeToolInput', () => {
  test('truncates long summaries', () => {
    const long = 'a'.repeat(200)
    const summary = summarizeToolInput({ command: long }, 40)
    expect(summary.length).toBe(40)
    expect(summary.endsWith('\u2026')).toBe(true)
  })
})

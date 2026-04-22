import { describe, expect, test } from 'bun:test'
import { resolveHotkey } from '../PermissionPromptOptions.js'

describe('resolveHotkey', () => {
  test('uses the backend-provided hotkey when present', () => {
    expect(resolveHotkey({ value: 'Allow', label: 'Allow', hotkey: 'y' })).toBe('y')
    expect(resolveHotkey({ value: 'Abc', label: 'Abc', hotkey: 'x' })).toBe('x')
  })

  test('infers y / n / a from well-known labels', () => {
    expect(resolveHotkey({ value: 'Allow', label: 'Allow' })).toBe('y')
    expect(resolveHotkey({ value: 'Yes', label: 'Yes' })).toBe('y')
    expect(resolveHotkey({ value: 'Deny', label: 'Deny' })).toBe('n')
    expect(resolveHotkey({ value: 'Reject', label: 'Reject' })).toBe('n')
    expect(resolveHotkey({ value: 'Always Allow', label: 'Always Allow' })).toBe('a')
  })

  test('returns undefined for unknown labels without a backend hotkey', () => {
    expect(resolveHotkey({ value: 'x', label: 'Only this time' })).toBeUndefined()
  })
})

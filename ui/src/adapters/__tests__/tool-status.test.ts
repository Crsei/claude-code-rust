import { describe, expect, test } from 'bun:test'
import { classifyToolStatus, mergeToolStatuses } from '../tool-status.js'

describe('classifyToolStatus', () => {
  test('returns cancelled when the output mentions interruption', () => {
    expect(classifyToolStatus('Interrupted by user', false)).toBe('cancelled')
    expect(classifyToolStatus('task aborted', false)).toBe('cancelled')
    expect(classifyToolStatus('Task was cancelled.', false)).toBe('cancelled')
  })

  test('returns error when the error flag is set and no cancellation is detected', () => {
    expect(classifyToolStatus('Boom', true)).toBe('error')
  })

  test('returns success otherwise', () => {
    expect(classifyToolStatus('ok', false)).toBe('success')
  })
})

describe('mergeToolStatuses', () => {
  test('prioritizes error over everything', () => {
    expect(mergeToolStatuses(['error', 'running', 'success'])).toBe('error')
  })

  test('falls through running -> pending -> cancelled -> success', () => {
    expect(mergeToolStatuses(['running', 'pending', 'success'])).toBe('running')
    expect(mergeToolStatuses(['pending', 'success'])).toBe('pending')
    expect(mergeToolStatuses(['cancelled', 'success'])).toBe('cancelled')
    expect(mergeToolStatuses(['success', 'success'])).toBe('success')
  })
})

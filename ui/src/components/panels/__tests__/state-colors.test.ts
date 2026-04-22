import { describe, expect, test } from 'bun:test'
import { isHealthyState, stateColor } from '../state-colors.js'

describe('stateColor', () => {
  test('maps healthy states to the green family', () => {
    expect(stateColor('running')).toBe('#A6E3A1')
    expect(stateColor('connected')).toBe('#A6E3A1')
    expect(stateColor('installed')).toBe('#A6E3A1')
    expect(stateColor('enabled')).toBe('#A6E3A1')
  })

  test('maps in-flight states to the amber family', () => {
    expect(stateColor('starting')).toBe('#F9E2AF')
    expect(stateColor('connecting')).toBe('#F9E2AF')
    expect(stateColor('reconnecting')).toBe('#F9E2AF')
  })

  test('maps dormant states to grey', () => {
    expect(stateColor('stopped')).toBe('#6C7086')
    expect(stateColor('disabled')).toBe('#6C7086')
    expect(stateColor('not_installed')).toBe('#6C7086')
  })

  test('maps failures to the red family', () => {
    expect(stateColor('error')).toBe('#F38BA8')
    expect(stateColor('failed')).toBe('#F38BA8')
    expect(stateColor('crashed')).toBe('#F38BA8')
  })

  test('is case-insensitive', () => {
    expect(stateColor('RUNNING')).toBe('#A6E3A1')
    expect(stateColor('Error')).toBe('#F38BA8')
  })

  test('falls back to grey for unknown or missing states', () => {
    expect(stateColor('pink')).toBe('#6C7086')
    expect(stateColor(undefined)).toBe('#6C7086')
    expect(stateColor(null)).toBe('#6C7086')
    expect(stateColor('')).toBe('#6C7086')
  })
})

describe('isHealthyState', () => {
  test('returns true for the green-family states', () => {
    expect(isHealthyState('running')).toBe(true)
    expect(isHealthyState('connected')).toBe(true)
    expect(isHealthyState('Ready')).toBe(true)
  })

  test('returns false for in-flight / dormant / error / missing states', () => {
    expect(isHealthyState('starting')).toBe(false)
    expect(isHealthyState('stopped')).toBe(false)
    expect(isHealthyState('error')).toBe(false)
    expect(isHealthyState(undefined)).toBe(false)
  })
})

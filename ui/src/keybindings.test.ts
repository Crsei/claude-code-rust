import { describe, expect, test } from 'bun:test'
import { matchesShortcut } from './keybindings.js'

describe('matchesShortcut', () => {
  test('matches global transcript and app shortcuts', () => {
    expect(matchesShortcut('app.toggleTranscript', 'o', { ctrl: true }, 'o')).toBe(true)
    expect(matchesShortcut('app.redraw', 'l', { ctrl: true }, 'l')).toBe(true)
    expect(matchesShortcut('app.abort', 'c', { ctrl: true }, 'c')).toBe(true)
    expect(matchesShortcut('app.quit', 'd', { ctrl: true }, 'd')).toBe(true)
    expect(matchesShortcut('app.toggleVim', 'g', { ctrl: true }, 'g')).toBe(true)
  })

  test('matches transcript exit with esc or q', () => {
    expect(matchesShortcut('transcript.exit', '', { escape: true }, 'escape')).toBe(true)
    expect(matchesShortcut('transcript.exit', 'q', {}, 'q')).toBe(true)
    expect(matchesShortcut('transcript.exit', 'q', { ctrl: true }, 'q')).toBe(false)
  })

  test('matches completion and navigation shortcuts', () => {
    expect(matchesShortcut('input.complete', '', { tab: true }, 'tab')).toBe(true)
    expect(matchesShortcut('input.confirm', '', { return: true }, 'enter')).toBe(true)
    expect(matchesShortcut('input.cancel', '', { escape: true }, 'escape')).toBe(true)
    expect(matchesShortcut('list.previous', '', { upArrow: true }, 'up')).toBe(true)
    expect(matchesShortcut('list.next', '', { downArrow: true }, 'down')).toBe(true)
  })

  test('does not treat plain text characters as ctrl shortcuts', () => {
    expect(matchesShortcut('app.toggleTranscript', 'o', { ctrl: false }, 'o')).toBe(false)
    expect(matchesShortcut('app.redraw', 'l', { ctrl: false }, 'l')).toBe(false)
    expect(matchesShortcut('app.quit', 'd', { ctrl: false }, 'd')).toBe(false)
    expect(matchesShortcut('app.toggleVim', 'g', { ctrl: false }, 'g')).toBe(false)
  })
})

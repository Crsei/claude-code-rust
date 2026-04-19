import { describe, expect, test } from 'bun:test'
import { matchesShortcut, resolveAction } from './keybindings.js'

describe('matchesShortcut (legacy names)', () => {
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

describe('matchesShortcut (spec-aligned names)', () => {
  test('app:interrupt and app:exit map to ctrl+c / ctrl+d', () => {
    expect(matchesShortcut('app:interrupt', 'c', { ctrl: true }, 'c')).toBe(true)
    expect(matchesShortcut('app:exit', 'd', { ctrl: true }, 'd')).toBe(true)
  })

  test('chat:submit and chat:cancel wire to enter / escape', () => {
    expect(matchesShortcut('chat:submit', '', { return: true }, 'enter')).toBe(true)
    expect(matchesShortcut('chat:cancel', '', { escape: true }, 'escape')).toBe(true)
  })

  test('autocomplete:accept fires on tab', () => {
    expect(matchesShortcut('autocomplete:accept', '', { tab: true }, 'tab')).toBe(true)
  })

  test('scroll:pageUp / scroll:pageDown handle PgUp/PgDn', () => {
    expect(matchesShortcut('scroll:pageUp', '', { pageUp: true }, 'pageup')).toBe(true)
    expect(matchesShortcut('scroll:pageDown', '', { pageDown: true }, 'pagedown')).toBe(true)
  })
})

describe('resolveAction', () => {
  test('maps legacy names to spec-aligned ones', () => {
    expect(resolveAction('app.abort')).toBe('app:interrupt')
    expect(resolveAction('app.quit')).toBe('app:exit')
    expect(resolveAction('input.confirm')).toBe('chat:submit')
    expect(resolveAction('input.cancel')).toBe('chat:cancel')
    expect(resolveAction('input.complete')).toBe('autocomplete:accept')
    expect(resolveAction('list.next')).toBe('select:next')
    expect(resolveAction('messages.pageUp')).toBe('scroll:pageUp')
  })

  test('passes spec-aligned names through unchanged', () => {
    expect(resolveAction('chat:submit')).toBe('chat:submit')
    expect(resolveAction('app:interrupt')).toBe('app:interrupt')
  })
})

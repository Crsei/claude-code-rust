import { describe, expect, test } from 'bun:test'
import {
  matchesShortcut,
  normalizeEditorMode,
  resolveAction,
  shortcutLabel,
  transcriptTitle,
  type KeybindingConfig,
} from './keybindings.js'

const overriddenConfig: KeybindingConfig = {
  bindings: [
    {
      context: 'Chat',
      bindings: {
        'ctrl+c': null,
        enter: 'chat:newline',
      },
    },
    {
      context: 'Scroll',
      bindings: {
        pageup: null,
        'ctrl+b': 'scroll:pageUp',
      },
    },
  ],
}

describe('matchesShortcut', () => {
  test('preserves legacy action aliases', () => {
    expect(matchesShortcut('app.abort', 'c', { ctrl: true }, 'c')).toBe(true)
    expect(matchesShortcut('app.quit', 'd', { ctrl: true }, 'd')).toBe(true)
    expect(matchesShortcut('transcript.exit', 'q', {}, 'q')).toBe(true)
  })

  test('honors context-local overrides', () => {
    expect(matchesShortcut('chat:submit', '', { return: true }, 'enter', {
      context: 'Chat',
      config: overriddenConfig,
    })).toBe(false)
    expect(matchesShortcut('chat:newline', '', { return: true }, 'enter', {
      context: 'Chat',
      config: overriddenConfig,
    })).toBe(true)
  })

  test('context-local unbind suppresses global fallback in that context', () => {
    expect(matchesShortcut('app:interrupt', 'c', { ctrl: true }, 'c', {
      context: 'Chat',
      config: overriddenConfig,
    })).toBe(false)
    expect(matchesShortcut('app:interrupt', 'c', { ctrl: true }, 'c', {
      context: 'Global',
      config: overriddenConfig,
    })).toBe(true)
  })

  test('uses context-specific defaults ahead of frontend fallback globals', () => {
    expect(matchesShortcut('chat:clearInput', 'l', { ctrl: true }, 'l', {
      context: 'Chat',
    })).toBe(true)
    expect(matchesShortcut('app:redraw', 'l', { ctrl: true }, 'l', {
      context: 'Chat',
    })).toBe(false)
    expect(shortcutLabel('chat:clearInput', { context: 'Chat' })).toBe('Ctrl+L')
  })

  test('supports overridden scroll bindings', () => {
    expect(matchesShortcut('scroll:pageUp', 'b', { ctrl: true }, 'b', {
      context: 'Scroll',
      config: overriddenConfig,
    })).toBe(true)
    expect(matchesShortcut('scroll:pageUp', '', { pageUp: true }, 'pageup', {
      context: 'Scroll',
      config: overriddenConfig,
    })).toBe(false)
  })
})

describe('labels and titles', () => {
  test('formats resolved labels from canonical bindings', () => {
    expect(shortcutLabel('app:toggleTranscript', { context: 'Global' })).toBe('Ctrl+O')
    expect(shortcutLabel('transcript:exit', { context: 'Transcript' })).toBe('Q / Esc')
    expect(transcriptTitle(overriddenConfig)).toContain('Ctrl+O')
  })
})

describe('helpers', () => {
  test('maps legacy names to canonical actions', () => {
    expect(resolveAction('app.abort')).toBe('app:interrupt')
    expect(resolveAction('input.complete')).toBe('autocomplete:accept')
    expect(resolveAction('messages.pageDown')).toBe('scroll:pageDown')
  })

  test('normalizes editor mode values', () => {
    expect(normalizeEditorMode('vim')).toBe('vim')
    expect(normalizeEditorMode('normal')).toBe('normal')
    expect(normalizeEditorMode('emacs')).toBe('normal')
    expect(normalizeEditorMode(null)).toBe('normal')
  })
})

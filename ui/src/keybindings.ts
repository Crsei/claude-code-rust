// Shared action vocabulary for the OpenTUI frontend.
//
// Aligned with the Claude Code spec (`namespace:action`) and the Rust
// registry in `src/keybindings/`. The canonical action names live in
// `src/keybindings/defaults.rs`; this file mirrors the subset that the
// frontend currently owns. When the Rust backend exposes its default set
// over IPC (future work tracked by issue #10), this hand-maintained list
// becomes a fallback.

export type ViewMode = 'prompt' | 'transcript'

/**
 * Canonical action identifier using the `namespace:action` format from the
 * spec. Aliases for legacy `app.abort`-style strings are still accepted at
 * the matcher (see `resolveAction`) so UI code can migrate incrementally.
 */
export type ShortcutAction =
  // -- Spec-aligned names -----------------------------------------------
  | 'app:interrupt'
  | 'app:exit'
  | 'app:toggleTodos'
  | 'app:toggleTranscript'
  | 'app:redraw'
  | 'chat:submit'
  | 'chat:cancel'
  | 'chat:newline'
  | 'chat:clearInput'
  | 'autocomplete:accept'
  | 'autocomplete:dismiss'
  | 'autocomplete:previous'
  | 'autocomplete:next'
  | 'select:previous'
  | 'select:next'
  | 'transcript:exit'
  | 'scroll:pageUp'
  | 'scroll:pageDown'
  | 'scroll:lineUp'
  | 'scroll:lineDown'
  | 'scroll:top'
  | 'scroll:bottom'
  | 'app:toggleVim'

/**
 * Legacy action aliases — kept for backward compatibility so components
 * that haven't migrated still work.
 */
const LEGACY_ALIASES: Record<string, ShortcutAction> = {
  'app.abort': 'app:interrupt',
  'app.quit': 'app:exit',
  'app.toggleVim': 'app:toggleVim',
  'app.toggleTranscript': 'app:toggleTranscript',
  'app.redraw': 'app:redraw',
  'transcript.exit': 'transcript:exit',
  'input.complete': 'autocomplete:accept',
  'input.confirm': 'chat:submit',
  'input.cancel': 'chat:cancel',
  'list.previous': 'select:previous',
  'list.next': 'select:next',
  'messages.pageUp': 'scroll:pageUp',
  'messages.pageDown': 'scroll:pageDown',
  'messages.scrollUp': 'scroll:lineUp',
  'messages.scrollDown': 'scroll:lineDown',
  'messages.top': 'scroll:top',
  'messages.bottom': 'scroll:bottom',
}

/** Resolve a caller-provided action string to a canonical ShortcutAction. */
export function resolveAction(action: string): ShortcutAction {
  if (action in LEGACY_ALIASES) {
    return LEGACY_ALIASES[action]
  }
  return action as ShortcutAction
}

export type KeyLike = {
  ctrl?: boolean
  meta?: boolean
  shift?: boolean
  return?: boolean
  escape?: boolean
  tab?: boolean
  pageUp?: boolean
  pageDown?: boolean
  upArrow?: boolean
  downArrow?: boolean
  home?: boolean
  end?: boolean
}

type ShortcutMatcher = {
  label: string
  match: (input: string, key: KeyLike, eventName?: string) => boolean
}

function ctrlChar(char: string): ShortcutMatcher {
  const upper = char.toUpperCase()
  return {
    label: `Ctrl+${upper}`,
    match: (input, key) => !!key.ctrl && !key.meta && input === char,
  }
}

const SHORTCUTS: Record<ShortcutAction, ShortcutMatcher[]> = {
  'app:interrupt': [ctrlChar('c')],
  'app:exit': [ctrlChar('d')],
  'app:toggleVim': [ctrlChar('g')],
  'app:toggleTranscript': [ctrlChar('o')],
  'app:toggleTodos': [ctrlChar('t')],
  'app:redraw': [ctrlChar('l')],
  'chat:submit': [
    {
      label: 'Enter',
      match: (_input, key) => !!key.return,
    },
  ],
  'chat:cancel': [
    {
      label: 'Esc',
      match: (_input, key) => !!key.escape,
    },
  ],
  'chat:newline': [ctrlChar('j')],
  'chat:clearInput': [ctrlChar('l')],
  'autocomplete:accept': [
    {
      label: 'Tab',
      match: (_input, key) => !!key.tab,
    },
  ],
  'autocomplete:dismiss': [
    {
      label: 'Esc',
      match: (_input, key) => !!key.escape,
    },
  ],
  'autocomplete:previous': [
    {
      label: 'Up',
      match: (_input, key, eventName) => eventName === 'up' || !!key.upArrow,
    },
  ],
  'autocomplete:next': [
    {
      label: 'Down',
      match: (_input, key, eventName) => eventName === 'down' || !!key.downArrow,
    },
  ],
  'select:previous': [
    {
      label: 'Up',
      match: (_input, key, eventName) => eventName === 'up' || !!key.upArrow,
    },
  ],
  'select:next': [
    {
      label: 'Down',
      match: (_input, key, eventName) => eventName === 'down' || !!key.downArrow,
    },
  ],
  'transcript:exit': [
    {
      label: 'Esc',
      match: (_input, key) => !!key.escape,
    },
    {
      label: 'q',
      match: (input, key) => input === 'q' && !key.ctrl && !key.meta,
    },
  ],
  'scroll:pageUp': [
    {
      label: 'PgUp',
      match: (_input, key, eventName) => eventName === 'pageup' || !!key.pageUp,
    },
  ],
  'scroll:pageDown': [
    {
      label: 'PgDn',
      match: (_input, key, eventName) => eventName === 'pagedown' || !!key.pageDown,
    },
  ],
  'scroll:lineUp': [
    {
      label: 'Ctrl+Up',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'up',
    },
  ],
  'scroll:lineDown': [
    {
      label: 'Ctrl+Down',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'down',
    },
  ],
  'scroll:top': [
    {
      label: 'Ctrl+Home',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'home',
    },
  ],
  'scroll:bottom': [
    {
      label: 'Ctrl+End',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'end',
    },
  ],
}

export function matchesShortcut(
  action: ShortcutAction | string,
  input: string,
  key: KeyLike,
  eventName?: string,
): boolean {
  const canonical = resolveAction(action)
  const matchers = SHORTCUTS[canonical]
  if (!matchers) {
    return false
  }
  return matchers.some(shortcut => shortcut.match(input, key, eventName))
}

export function shortcutLabel(action: ShortcutAction | string): string {
  const canonical = resolveAction(action)
  const matchers = SHORTCUTS[canonical]
  if (!matchers) {
    return ''
  }
  return matchers.map(shortcut => shortcut.label).join(' / ')
}

export function transcriptTitle(): string {
  return `Transcript | ${shortcutLabel('app:toggleTranscript')} prompt | ${shortcutLabel('transcript:exit')} exit`
}

export function messageScrollHint(): string {
  return [
    shortcutLabel('scroll:pageUp'),
    shortcutLabel('scroll:pageDown'),
    shortcutLabel('scroll:lineUp'),
    shortcutLabel('scroll:lineDown'),
  ].join(' ')
}

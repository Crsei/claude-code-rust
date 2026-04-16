export type ViewMode = 'prompt' | 'transcript'

export type ShortcutAction =
  | 'app.abort'
  | 'app.quit'
  | 'app.toggleVim'
  | 'app.toggleTranscript'
  | 'app.redraw'
  | 'transcript.exit'
  | 'input.complete'
  | 'input.confirm'
  | 'input.cancel'
  | 'list.previous'
  | 'list.next'
  | 'messages.pageUp'
  | 'messages.pageDown'
  | 'messages.scrollUp'
  | 'messages.scrollDown'
  | 'messages.top'
  | 'messages.bottom'

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
  'app.abort': [ctrlChar('c')],
  'app.quit': [ctrlChar('d')],
  'app.toggleVim': [ctrlChar('g')],
  'app.toggleTranscript': [ctrlChar('o')],
  'app.redraw': [ctrlChar('l')],
  'transcript.exit': [
    {
      label: 'Esc',
      match: (_input, key) => !!key.escape,
    },
    {
      label: 'q',
      match: (input, key) => input === 'q' && !key.ctrl && !key.meta,
    },
  ],
  'input.complete': [
    {
      label: 'Tab',
      match: (_input, key) => !!key.tab,
    },
  ],
  'input.confirm': [
    {
      label: 'Enter',
      match: (_input, key) => !!key.return,
    },
  ],
  'input.cancel': [
    {
      label: 'Esc',
      match: (_input, key) => !!key.escape,
    },
  ],
  'list.previous': [
    {
      label: 'Up',
      match: (_input, key, eventName) => eventName === 'up' || !!key.upArrow,
    },
  ],
  'list.next': [
    {
      label: 'Down',
      match: (_input, key, eventName) => eventName === 'down' || !!key.downArrow,
    },
  ],
  'messages.pageUp': [
    {
      label: 'PgUp',
      match: (_input, key, eventName) => eventName === 'pageup' || !!key.pageUp,
    },
  ],
  'messages.pageDown': [
    {
      label: 'PgDn',
      match: (_input, key, eventName) => eventName === 'pagedown' || !!key.pageDown,
    },
  ],
  'messages.scrollUp': [
    {
      label: 'Ctrl+Up',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'up',
    },
  ],
  'messages.scrollDown': [
    {
      label: 'Ctrl+Down',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'down',
    },
  ],
  'messages.top': [
    {
      label: 'Ctrl+Home',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'home',
    },
  ],
  'messages.bottom': [
    {
      label: 'Ctrl+End',
      match: (_input, key, eventName) => !!key.ctrl && eventName === 'end',
    },
  ],
}

export function matchesShortcut(
  action: ShortcutAction,
  input: string,
  key: KeyLike,
  eventName?: string,
): boolean {
  return SHORTCUTS[action].some(shortcut => shortcut.match(input, key, eventName))
}

export function shortcutLabel(action: ShortcutAction): string {
  return SHORTCUTS[action].map(shortcut => shortcut.label).join(' / ')
}

export function transcriptTitle(): string {
  return `Transcript | ${shortcutLabel('app.toggleTranscript')} prompt | ${shortcutLabel('transcript.exit')} exit`
}

export function messageScrollHint(): string {
  return [
    shortcutLabel('messages.pageUp'),
    shortcutLabel('messages.pageDown'),
    shortcutLabel('messages.scrollUp'),
    shortcutLabel('messages.scrollDown'),
  ].join(' ')
}

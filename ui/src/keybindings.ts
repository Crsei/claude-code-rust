// Shared keybinding vocabulary and config resolver for the OpenTUI frontend.
//
// The canonical built-in table mirrors the Rust registry in
// `src/keybindings/defaults.rs`, with a small frontend-only fallback tail for
// actions that are still handled locally until the leader wires hotspot IPC.

export type ViewMode = 'prompt' | 'transcript'
export type EditorMode = 'normal' | 'vim'
export type ShortcutAction = `${string}:${string}`
export type ShortcutContext = string

export interface KeybindingConfigBlock {
  context: ShortcutContext
  bindings: Record<string, string | null>
}

export interface KeybindingConfig {
  bindings?: KeybindingConfigBlock[]
}

export type KeyLike = {
  ctrl?: boolean
  meta?: boolean
  alt?: boolean
  shift?: boolean
  return?: boolean
  escape?: boolean
  tab?: boolean
  pageUp?: boolean
  pageDown?: boolean
  upArrow?: boolean
  downArrow?: boolean
  leftArrow?: boolean
  rightArrow?: boolean
  home?: boolean
  end?: boolean
  backspace?: boolean
  delete?: boolean
}

type BindingEntry = {
  context: ShortcutContext
  chord: string
  action: ShortcutAction
}

type MatchOptions = {
  context?: ShortcutContext
  config?: KeybindingConfig | null
}

type CompiledKeybindings = {
  bindingsByContext: Map<ShortcutContext, Map<string, ShortcutAction>>
  blockedByContext: Map<ShortcutContext, string[]>
}

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

const BUILTIN_BINDINGS: BindingEntry[] = [
  { context: 'Global', chord: 'ctrl+c', action: 'app:interrupt' },
  { context: 'Global', chord: 'ctrl+d', action: 'app:exit' },
  { context: 'Global', chord: 'ctrl+t', action: 'app:toggleTodos' },
  { context: 'Global', chord: 'ctrl+o', action: 'app:toggleTranscript' },
  { context: 'Global', chord: 'ctrl+r', action: 'history:search' },
  { context: 'Global', chord: 'up', action: 'history:previous' },
  { context: 'Global', chord: 'down', action: 'history:next' },
  { context: 'Chat', chord: 'escape', action: 'chat:cancel' },
  { context: 'Chat', chord: 'ctrl+l', action: 'chat:clearInput' },
  { context: 'Chat', chord: 'ctrl+x ctrl+k', action: 'chat:killAgents' },
  { context: 'Chat', chord: 'shift+tab', action: 'chat:cycleMode' },
  { context: 'Chat', chord: 'meta+p', action: 'chat:modelPicker' },
  { context: 'Chat', chord: 'meta+o', action: 'chat:fastMode' },
  { context: 'Chat', chord: 'meta+t', action: 'chat:thinkingToggle' },
  { context: 'Chat', chord: 'enter', action: 'chat:submit' },
  { context: 'Chat', chord: 'ctrl+j', action: 'chat:newline' },
  { context: 'Chat', chord: 'ctrl+g', action: 'chat:externalEditor' },
  { context: 'Chat', chord: 'ctrl+x ctrl+e', action: 'chat:externalEditor' },
  { context: 'Chat', chord: 'ctrl+s', action: 'chat:stash' },
  { context: 'Chat', chord: 'ctrl+v', action: 'chat:imagePaste' },
  { context: 'Chat', chord: 'ctrl+space', action: 'voice:pushToTalk' },
  { context: 'Autocomplete', chord: 'tab', action: 'autocomplete:accept' },
  { context: 'Autocomplete', chord: 'escape', action: 'autocomplete:dismiss' },
  { context: 'Autocomplete', chord: 'up', action: 'autocomplete:previous' },
  { context: 'Autocomplete', chord: 'down', action: 'autocomplete:next' },
  { context: 'Confirmation', chord: 'y', action: 'confirm:yes' },
  { context: 'Confirmation', chord: 'enter', action: 'confirm:yes' },
  { context: 'Confirmation', chord: 'n', action: 'confirm:no' },
  { context: 'Confirmation', chord: 'escape', action: 'confirm:no' },
  { context: 'Confirmation', chord: 'up', action: 'confirm:previous' },
  { context: 'Confirmation', chord: 'down', action: 'confirm:next' },
  { context: 'Confirmation', chord: 'tab', action: 'confirm:nextField' },
  { context: 'Confirmation', chord: 'space', action: 'confirm:toggle' },
  { context: 'Confirmation', chord: 'shift+tab', action: 'confirm:cycleMode' },
  { context: 'Confirmation', chord: 'ctrl+e', action: 'confirm:toggleExplanation' },
  { context: 'Transcript', chord: 'ctrl+e', action: 'transcript:toggleShowAll' },
  { context: 'Transcript', chord: 'q', action: 'transcript:exit' },
  { context: 'Transcript', chord: 'escape', action: 'transcript:exit' },
  { context: 'HistorySearch', chord: 'ctrl+r', action: 'historySearch:next' },
  { context: 'HistorySearch', chord: 'escape', action: 'historySearch:accept' },
  { context: 'HistorySearch', chord: 'tab', action: 'historySearch:accept' },
  { context: 'HistorySearch', chord: 'ctrl+c', action: 'historySearch:cancel' },
  { context: 'HistorySearch', chord: 'enter', action: 'historySearch:execute' },
  { context: 'Task', chord: 'ctrl+b', action: 'task:background' },
  { context: 'ThemePicker', chord: 'ctrl+t', action: 'theme:toggleSyntaxHighlighting' },
  { context: 'Help', chord: 'escape', action: 'help:dismiss' },
  { context: 'Tabs', chord: 'tab', action: 'tabs:next' },
  { context: 'Tabs', chord: 'right', action: 'tabs:next' },
  { context: 'Tabs', chord: 'shift+tab', action: 'tabs:previous' },
  { context: 'Tabs', chord: 'left', action: 'tabs:previous' },
  { context: 'Attachments', chord: 'right', action: 'attachments:next' },
  { context: 'Attachments', chord: 'left', action: 'attachments:previous' },
  { context: 'Attachments', chord: 'backspace', action: 'attachments:remove' },
  { context: 'Attachments', chord: 'delete', action: 'attachments:remove' },
  { context: 'Attachments', chord: 'escape', action: 'attachments:exit' },
  { context: 'Footer', chord: 'right', action: 'footer:next' },
  { context: 'Footer', chord: 'left', action: 'footer:previous' },
  { context: 'Footer', chord: 'up', action: 'footer:up' },
  { context: 'Footer', chord: 'down', action: 'footer:down' },
  { context: 'Footer', chord: 'enter', action: 'footer:openSelected' },
  { context: 'Footer', chord: 'escape', action: 'footer:clearSelection' },
  { context: 'MessageSelector', chord: 'up', action: 'messageSelector:up' },
  { context: 'MessageSelector', chord: 'k', action: 'messageSelector:up' },
  { context: 'MessageSelector', chord: 'ctrl+p', action: 'messageSelector:up' },
  { context: 'MessageSelector', chord: 'down', action: 'messageSelector:down' },
  { context: 'MessageSelector', chord: 'j', action: 'messageSelector:down' },
  { context: 'MessageSelector', chord: 'ctrl+n', action: 'messageSelector:down' },
  { context: 'MessageSelector', chord: 'enter', action: 'messageSelector:select' },
  { context: 'DiffDialog', chord: 'escape', action: 'diff:dismiss' },
  { context: 'DiffDialog', chord: 'left', action: 'diff:previousSource' },
  { context: 'DiffDialog', chord: 'right', action: 'diff:nextSource' },
  { context: 'DiffDialog', chord: 'up', action: 'diff:previousFile' },
  { context: 'DiffDialog', chord: 'down', action: 'diff:nextFile' },
  { context: 'DiffDialog', chord: 'enter', action: 'diff:viewDetails' },
  { context: 'ModelPicker', chord: 'left', action: 'modelPicker:decreaseEffort' },
  { context: 'ModelPicker', chord: 'right', action: 'modelPicker:increaseEffort' },
  { context: 'Select', chord: 'down', action: 'select:next' },
  { context: 'Select', chord: 'j', action: 'select:next' },
  { context: 'Select', chord: 'ctrl+n', action: 'select:next' },
  { context: 'Select', chord: 'up', action: 'select:previous' },
  { context: 'Select', chord: 'k', action: 'select:previous' },
  { context: 'Select', chord: 'ctrl+p', action: 'select:previous' },
  { context: 'Select', chord: 'enter', action: 'select:accept' },
  { context: 'Select', chord: 'escape', action: 'select:cancel' },
  { context: 'Plugin', chord: 'space', action: 'plugin:toggle' },
  { context: 'Plugin', chord: 'i', action: 'plugin:install' },
  { context: 'Settings', chord: '/', action: 'settings:search' },
  { context: 'Settings', chord: 'r', action: 'settings:retry' },
  { context: 'Settings', chord: 'enter', action: 'settings:close' },
  { context: 'Doctor', chord: 'f', action: 'doctor:fix' },
  { context: 'Scroll', chord: 'pageup', action: 'scroll:pageUp' },
  { context: 'Scroll', chord: 'pagedown', action: 'scroll:pageDown' },
  { context: 'Scroll', chord: 'ctrl+home', action: 'scroll:top' },
  { context: 'Scroll', chord: 'ctrl+end', action: 'scroll:bottom' },
  { context: 'Scroll', chord: 'ctrl+shift+c', action: 'selection:copy' },
  // Frontend-only fallback entries until the leader wires backend payloads in
  // the remaining hotspots. These are intentionally isolated here so the UI
  // consumes one table instead of scattered matcher code.
  { context: 'Global', chord: 'ctrl+g', action: 'app:toggleVim' },
  { context: 'Global', chord: 'ctrl+l', action: 'app:redraw' },
  { context: 'Scroll', chord: 'ctrl+up', action: 'scroll:lineUp' },
  { context: 'Scroll', chord: 'ctrl+down', action: 'scroll:lineDown' },
]

const DEFAULT_COMPILED = compileKeybindingsInternal(null)
const configCache = new WeakMap<KeybindingConfig, CompiledKeybindings>()

/** Resolve a caller-provided action string to a canonical ShortcutAction. */
export function resolveAction(action: string): ShortcutAction {
  if (action in LEGACY_ALIASES) {
    return LEGACY_ALIASES[action]
  }
  return action as ShortcutAction
}

export function normalizeEditorMode(mode?: string | null): EditorMode {
  return mode?.trim().toLowerCase() === 'vim' ? 'vim' : 'normal'
}

export function matchesShortcut(
  action: ShortcutAction | string,
  input: string,
  key: KeyLike,
  eventName?: string,
  options?: MatchOptions,
): boolean {
  const stroke = eventToStroke(input, key, eventName)
  if (!stroke) {
    return false
  }

  return bindingsForAction(action, options).some(chord => isSingleStroke(chord) && chord === stroke)
}

export function shortcutLabel(
  action: ShortcutAction | string,
  options?: MatchOptions,
): string {
  return bindingsForAction(action, options).map(formatChordLabel).join(' / ')
}

export function transcriptTitle(config?: KeybindingConfig | null): string {
  return `Transcript | ${shortcutLabel('app:toggleTranscript', { context: 'Global', config })} prompt | ${shortcutLabel('transcript:exit', { context: 'Transcript', config })} exit`
}

export function messageScrollHint(config?: KeybindingConfig | null): string {
  return [
    shortcutLabel('scroll:pageUp', { context: 'Scroll', config }),
    shortcutLabel('scroll:pageDown', { context: 'Scroll', config }),
    shortcutLabel('scroll:lineUp', { context: 'Scroll', config }),
    shortcutLabel('scroll:lineDown', { context: 'Scroll', config }),
  ].join(' ')
}

function bindingsForAction(
  action: ShortcutAction | string,
  options?: MatchOptions,
): string[] {
  const canonicalAction = resolveAction(action)
  const compiled = compileKeybindings(options?.config ?? null)
  if (!options?.context) {
    const allChords: string[] = []
    for (const map of compiled.bindingsByContext.values()) {
      for (const [chord, boundAction] of map.entries()) {
        if (boundAction === canonicalAction && !allChords.includes(chord)) {
          allChords.push(chord)
        }
      }
    }
    return allChords
  }

  const context = options.context
  const localBindings = compiled.bindingsByContext.get(context) ?? new Map()
  const localChords = [...localBindings.entries()]
    .filter(([, boundAction]) => boundAction === canonicalAction)
    .map(([chord]) => chord)
  if (context === 'Global') {
    return localChords
  }

  const blocked = compiled.blockedByContext.get(context) ?? []
  const localChordSet = new Set(localBindings.keys())
  const globalBindings = compiled.bindingsByContext.get('Global') ?? new Map()
  const globalChords = [...globalBindings.entries()]
    .filter(([chord, boundAction]) =>
      boundAction === canonicalAction
        && !localChordSet.has(chord)
        && !blocked.some(prefix => chordHasPrefix(chord, prefix)),
    )
    .map(([chord]) => chord)

  return [...localChords, ...globalChords]
}

function compileKeybindings(config?: KeybindingConfig | null): CompiledKeybindings {
  if (!config) {
    return DEFAULT_COMPILED
  }

  const cached = configCache.get(config)
  if (cached) {
    return cached
  }

  const compiled = compileKeybindingsInternal(config)
  configCache.set(config, compiled)
  return compiled
}

function compileKeybindingsInternal(config?: KeybindingConfig | null): CompiledKeybindings {
  const bindingsByContext = new Map<ShortcutContext, Map<string, ShortcutAction>>()
  const blockedByContext = new Map<ShortcutContext, string[]>()

  for (const entry of BUILTIN_BINDINGS) {
    getContextMap(bindingsByContext, entry.context).set(normalizeChord(entry.chord), entry.action)
  }

  for (const block of config?.bindings ?? []) {
    const context = normalizeContext(block.context)
    const contextMap = getContextMap(bindingsByContext, context)
    const blocked = blockedByContext.get(context) ?? []

    for (const [rawChord, rawValue] of Object.entries(block.bindings ?? {})) {
      const chord = normalizeChord(rawChord)
      if (!chord) {
        continue
      }

      if (rawValue === null) {
        for (const existingChord of [...contextMap.keys()]) {
          if (chordHasPrefix(existingChord, chord)) {
            contextMap.delete(existingChord)
          }
        }
        blocked.push(chord)
        continue
      }

      contextMap.set(chord, resolveAction(rawValue))
    }

    if (blocked.length > 0) {
      blockedByContext.set(context, blocked)
    }
  }

  return { bindingsByContext, blockedByContext }
}

function getContextMap(
  bindingsByContext: Map<ShortcutContext, Map<string, ShortcutAction>>,
  context: ShortcutContext,
): Map<string, ShortcutAction> {
  const existing = bindingsByContext.get(context)
  if (existing) {
    return existing
  }

  const created = new Map<string, ShortcutAction>()
  bindingsByContext.set(context, created)
  return created
}

function normalizeContext(context: ShortcutContext): ShortcutContext {
  return context.trim() || 'Global'
}

function normalizeChord(rawChord: string): string {
  return rawChord
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .map(stroke => stroke.split('+').map(part => part.trim()).filter(Boolean).join('+'))
    .filter(Boolean)
    .join(' ')
}

function chordHasPrefix(chord: string, prefix: string): boolean {
  return chord === prefix || chord.startsWith(`${prefix} `)
}

function isSingleStroke(chord: string): boolean {
  return !chord.includes(' ')
}

function eventToStroke(input: string, key: KeyLike, eventName?: string): string | null {
  const base = resolveBaseKey(input, key, eventName)
  if (!base) {
    return null
  }

  const parts: string[] = []
  if (key.ctrl) parts.push('ctrl')
  if (key.meta) parts.push('meta')
  if (key.alt) parts.push('alt')
  if (shouldApplyShift(input, key, base)) parts.push('shift')
  parts.push(base)
  return normalizeChord(parts.join('+'))
}

function resolveBaseKey(input: string, key: KeyLike, eventName?: string): string | null {
  const loweredInput = input.length === 1 ? input.toLowerCase() : input
  const lowerEventName = eventName?.toLowerCase()

  if (key.return) return 'enter'
  if (key.escape) return 'escape'
  if (key.tab) return 'tab'
  if (key.pageUp) return 'pageup'
  if (key.pageDown) return 'pagedown'
  if (key.upArrow) return 'up'
  if (key.downArrow) return 'down'
  if (key.leftArrow) return 'left'
  if (key.rightArrow) return 'right'
  if (key.home) return 'home'
  if (key.end) return 'end'
  if (key.backspace) return 'backspace'
  if (key.delete) return 'delete'
  if (input === ' ') return 'space'
  if (loweredInput.length === 1) return loweredInput

  switch (lowerEventName) {
    case 'enter':
    case 'return':
      return 'enter'
    case 'escape':
      return 'escape'
    case 'tab':
      return 'tab'
    case 'pageup':
      return 'pageup'
    case 'pagedown':
      return 'pagedown'
    case 'up':
      return 'up'
    case 'down':
      return 'down'
    case 'left':
      return 'left'
    case 'right':
      return 'right'
    case 'home':
      return 'home'
    case 'end':
      return 'end'
    case 'backspace':
      return 'backspace'
    case 'delete':
      return 'delete'
    default:
      return lowerEventName?.length === 1 ? lowerEventName : null
  }
}

function shouldApplyShift(input: string, key: KeyLike, base: string): boolean {
  if (!key.shift) {
    return false
  }
  if (input.length === 1 && input >= 'A' && input <= 'Z') {
    return true
  }
  return base.length > 1 || !!key.ctrl || !!key.meta || !!key.alt
}

function formatChordLabel(chord: string): string {
  return chord
    .split(' ')
    .map(stroke => stroke.split('+').map(formatChordPart).join('+'))
    .join(' ')
}

function formatChordPart(part: string): string {
  switch (part) {
    case 'ctrl':
      return 'Ctrl'
    case 'meta':
      return 'Meta'
    case 'alt':
      return 'Alt'
    case 'shift':
      return 'Shift'
    case 'enter':
      return 'Enter'
    case 'escape':
      return 'Esc'
    case 'tab':
      return 'Tab'
    case 'pageup':
      return 'PgUp'
    case 'pagedown':
      return 'PgDn'
    case 'up':
      return 'Up'
    case 'down':
      return 'Down'
    case 'left':
      return 'Left'
    case 'right':
      return 'Right'
    case 'home':
      return 'Home'
    case 'end':
      return 'End'
    case 'backspace':
      return 'Backspace'
    case 'delete':
      return 'Delete'
    case 'space':
      return 'Space'
    default:
      return part.length === 1 ? part.toUpperCase() : part
  }
}

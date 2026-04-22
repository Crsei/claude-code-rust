import type { KeyEvent } from '@opentui/core'
import type { KeyLike } from '../../keybindings.js'
import { isPlainTextInput } from './utils.js'

export type ShortcutKey = KeyLike & {
  backspace?: boolean
  delete?: boolean
  wheelUp?: boolean
  wheelDown?: boolean
}

export function extractInput(event: KeyEvent): string {
  const seq = event.sequence ?? ''
  if (seq.length === 1 && seq.charCodeAt(0) >= 32) return seq
  if (!event.ctrl && !event.meta && isPlainTextInput(seq)) return seq
  if (event.ctrl && (event.name?.length ?? 0) === 1) return event.name ?? ''
  if ((event.name?.length ?? 0) === 1 && !event.ctrl && !event.meta) return event.name ?? ''
  return ''
}

export function toShortcutKey(event: KeyEvent): ShortcutKey {
  const name = event.name ?? ''
  return {
    ctrl: event.ctrl ?? false,
    meta: event.meta ?? false,
    alt: event.alt ?? false,
    shift: event.shift ?? false,
    return: name === 'return' || name === 'enter',
    escape: name === 'escape',
    tab: name === 'tab',
    leftArrow: name === 'left',
    rightArrow: name === 'right',
    upArrow: name === 'up',
    downArrow: name === 'down',
    pageUp: name === 'pageup',
    pageDown: name === 'pagedown',
    home: name === 'home',
    end: name === 'end',
    backspace: name === 'backspace',
    delete: name === 'delete',
    wheelUp: name === 'wheel_up' || name === 'scrollup',
    wheelDown: name === 'wheel_down' || name === 'scrolldown',
  }
}

export function formatWorkedDuration(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000))
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return minutes > 0 ? `${minutes}m ${seconds}s` : `${seconds}s`
}

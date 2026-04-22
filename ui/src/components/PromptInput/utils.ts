import { truncate } from '../../utils.js'

const PASTE_DETECT_CHARS = 100

export function isPasteInput(inputLength: number): boolean {
  return inputLength >= PASTE_DETECT_CHARS
}

export function formatPasteSize(text: string): string {
  const bytes = Buffer.byteLength(text, 'utf8')
  const kb = bytes / 1024
  return kb < 10 ? `pasted text ${Number(kb.toFixed(1))}kb` : `pasted text ${Math.round(kb)}kb`
}

export function promptPlaceholder(isBusy: boolean): string {
  return isBusy ? ' Working... draft the next message' : ' Type a message or /command...'
}

export function summarizeQueuedSubmissions(
  items: Array<{ text: string }>,
  maxVisible = 2,
): string {
  const visible = items
    .slice(0, maxVisible)
    .map(item => truncate(item.text.replace(/\s+/g, ' ').trim(), 36))
  if (items.length <= maxVisible) {
    return visible.join(' | ')
  }
  return `${visible.join(' | ')} | +${items.length - maxVisible} more`
}

export function isPlainTextInput(value: string): boolean {
  if (!value) {
    return false
  }

  for (const char of value) {
    if (char === '\n' || char === '\r' || char === '\t') {
      continue
    }
    if (char < ' ') {
      return false
    }
  }

  return true
}

export function insertAtCursor(text: string, cursorPos: number, value: string) {
  return {
    text: text.slice(0, cursorPos) + value + text.slice(cursorPos),
    cursorPos: cursorPos + value.length,
  }
}

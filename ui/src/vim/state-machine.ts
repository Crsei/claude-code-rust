import type { VimMode, VimAction, PendingOp } from './types.js'
import {
  prevCharPos, nextCharPos, wordEndPos, wordStartPos,
  wordEndInclusivePos, firstNonWhitespace,
} from './motions.js'

export class VimState {
  mode: VimMode = 'normal'
  enabled: boolean = false
  pendingOp: PendingOp = null
  repeatCount: number | null = null
  register: string = ''
  visualAnchor: number = 0

  enable(): void {
    this.enabled = true
    this.mode = 'normal'
    this.pendingOp = null
    this.repeatCount = null
  }

  disable(): void {
    this.enabled = false
    this.mode = 'insert'
    this.pendingOp = null
    this.repeatCount = null
  }

  toggle(): void {
    if (this.enabled) this.disable()
    else this.enable()
  }

  get indicator(): string {
    switch (this.mode) {
      case 'normal': return 'NORMAL'
      case 'insert': return 'INSERT'
      case 'visual': return 'VISUAL'
    }
  }

  private count(): number {
    const c = this.repeatCount ?? 1
    this.repeatCount = null
    return c
  }

  /** Process a key event. Returns a VimAction. */
  handleKey(
    input: string,       // raw char or empty
    key: {               // parsed key object from useInput
      ctrl?: boolean
      meta?: boolean
      shift?: boolean
      escape?: boolean
      return?: boolean
      backspace?: boolean
      delete?: boolean
      leftArrow?: boolean
      rightArrow?: boolean
      upArrow?: boolean
      downArrow?: boolean
      tab?: boolean
    },
    text: string,        // current input text
    cursor: number,      // current cursor position
  ): VimAction {
    if (!this.enabled) return { type: 'passthrough' }

    switch (this.mode) {
      case 'normal': return this.handleNormal(input, key, text, cursor)
      case 'insert': return this.handleInsert(input, key)
      case 'visual': return this.handleVisual(input, key, text, cursor)
    }
  }

  private handleNormal(
    input: string,
    key: any,
    text: string,
    cursor: number,
  ): VimAction {
    // Ctrl combos pass through
    if (key.ctrl) return { type: 'passthrough' }

    // Digit prefix
    if (input >= '1' && input <= '9') {
      const digit = parseInt(input)
      this.repeatCount = (this.repeatCount ?? 0) * 10 + digit
      return { type: 'none' }
    }
    if (input === '0' && this.repeatCount !== null) {
      this.repeatCount = this.repeatCount * 10
      return { type: 'none' }
    }

    // Mode switching
    if (input === 'i') {
      this.mode = 'insert'; this.pendingOp = null; this.repeatCount = null
      return { type: 'switch_mode', mode: 'insert' }
    }
    if (input === 'a') {
      this.mode = 'insert'; this.pendingOp = null; this.repeatCount = null
      const newPos = nextCharPos(text, cursor)
      if (newPos !== cursor) return { type: 'move_cursor', pos: newPos }
      return { type: 'switch_mode', mode: 'insert' }
    }
    if (input === 'I') {
      this.mode = 'insert'
      return { type: 'move_cursor', pos: 0 }
    }
    if (input === 'A') {
      this.mode = 'insert'
      return { type: 'move_cursor', pos: text.length }
    }
    if (input === 'v') {
      this.mode = 'visual'; this.visualAnchor = cursor
      return { type: 'switch_mode', mode: 'visual' }
    }

    // Navigation
    if (input === 'h' || key.leftArrow) {
      const count = this.count()
      let pos = cursor
      for (let i = 0; i < count; i++) pos = prevCharPos(text, pos)
      return { type: 'move_cursor', pos }
    }
    if (input === 'l' || key.rightArrow) {
      const count = this.count()
      let pos = cursor
      for (let i = 0; i < count; i++) pos = nextCharPos(text, pos)
      return { type: 'move_cursor', pos }
    }
    if (input === '0') return { type: 'move_cursor', pos: 0 }
    if (input === '$') return { type: 'move_cursor', pos: text.length }
    if (input === '^') return { type: 'move_cursor', pos: firstNonWhitespace(text) }

    // Word motions
    if (input === 'w') {
      if (this.pendingOp === 'delete') {
        this.pendingOp = null
        return { type: 'delete', start: cursor, end: wordEndPos(text, cursor) }
      }
      if (this.pendingOp === 'yank') {
        this.pendingOp = null
        return { type: 'yank', start: cursor, end: wordEndPos(text, cursor) }
      }
      if (this.pendingOp === 'change') {
        this.pendingOp = null; this.mode = 'insert'
        return { type: 'delete', start: cursor, end: wordEndPos(text, cursor) }
      }
      const count = this.count()
      let pos = cursor
      for (let i = 0; i < count; i++) pos = wordEndPos(text, pos)
      return { type: 'move_cursor', pos }
    }
    if (input === 'b') {
      const count = this.count()
      let pos = cursor
      for (let i = 0; i < count; i++) pos = wordStartPos(text, pos)
      return { type: 'move_cursor', pos }
    }
    if (input === 'e') {
      const count = this.count()
      let pos = cursor
      for (let i = 0; i < count; i++) pos = wordEndInclusivePos(text, pos)
      return { type: 'move_cursor', pos }
    }

    // Operators
    if (input === 'd') {
      if (this.pendingOp === 'delete') { this.pendingOp = null; return { type: 'delete_line' } }
      this.pendingOp = 'delete'; return { type: 'none' }
    }
    if (input === 'y') {
      if (this.pendingOp === 'yank') {
        this.pendingOp = null
        this.register = text
        return { type: 'yank_line' }
      }
      this.pendingOp = 'yank'; return { type: 'none' }
    }
    if (input === 'c') {
      if (this.pendingOp === 'change') {
        this.pendingOp = null; this.mode = 'insert'
        return { type: 'delete_line' }
      }
      this.pendingOp = 'change'; return { type: 'none' }
    }

    // Single-key operations
    if (input === 'x') {
      const end = nextCharPos(text, cursor)
      if (end > cursor) {
        this.register = text.slice(cursor, end)
        return { type: 'delete', start: cursor, end }
      }
      return { type: 'none' }
    }
    if (input === 'X') {
      const start = prevCharPos(text, cursor)
      if (start < cursor) {
        this.register = text.slice(start, cursor)
        return { type: 'delete', start, end: cursor }
      }
      return { type: 'none' }
    }
    if (input === 'p') {
      return this.register ? { type: 'paste', text: this.register } : { type: 'none' }
    }
    if (input === 'u') return { type: 'undo' }
    if (input === 'D') {
      if (cursor < text.length) {
        this.register = text.slice(cursor)
        return { type: 'delete', start: cursor, end: text.length }
      }
      return { type: 'none' }
    }
    if (input === 'C') {
      this.mode = 'insert'
      if (cursor < text.length) return { type: 'delete', start: cursor, end: text.length }
      return { type: 'switch_mode', mode: 'insert' }
    }

    // Submit
    if (key.return) return { type: 'submit' }

    // Escape resets
    if (key.escape) {
      this.pendingOp = null; this.repeatCount = null
      return { type: 'none' }
    }

    return { type: 'none' }
  }

  private handleInsert(input: string, key: any): VimAction {
    if (key.escape) {
      this.mode = 'normal'
      return { type: 'switch_mode', mode: 'normal' }
    }
    return { type: 'passthrough' }
  }

  private handleVisual(
    input: string,
    key: any,
    text: string,
    cursor: number,
  ): VimAction {
    if (key.escape) {
      this.mode = 'normal'
      return { type: 'switch_mode', mode: 'normal' }
    }

    // Navigation extends selection
    if (input === 'h' || key.leftArrow) return { type: 'move_cursor', pos: prevCharPos(text, cursor) }
    if (input === 'l' || key.rightArrow) return { type: 'move_cursor', pos: nextCharPos(text, cursor) }
    if (input === 'w') return { type: 'move_cursor', pos: wordEndPos(text, cursor) }
    if (input === 'b') return { type: 'move_cursor', pos: wordStartPos(text, cursor) }
    if (input === '0') return { type: 'move_cursor', pos: 0 }
    if (input === '$') return { type: 'move_cursor', pos: text.length }

    // Selection operations
    const [start, end] = this.selectionRange(cursor)

    if (input === 'd' || input === 'x') {
      this.mode = 'normal'
      this.register = text.slice(start, end)
      return { type: 'delete', start, end }
    }
    if (input === 'y') {
      this.mode = 'normal'
      this.register = text.slice(start, end)
      return { type: 'yank', start, end }
    }
    if (input === 'c') {
      this.mode = 'insert'
      return { type: 'delete', start, end }
    }

    return { type: 'none' }
  }

  private selectionRange(cursor: number): [number, number] {
    return cursor < this.visualAnchor
      ? [cursor, this.visualAnchor]
      : [this.visualAnchor, cursor]
  }
}

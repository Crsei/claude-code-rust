export type VimMode = 'normal' | 'insert' | 'visual'

export type VimAction =
  | { type: 'none' }
  | { type: 'insert_char'; char: string }
  | { type: 'delete'; start: number; end: number }
  | { type: 'move_cursor'; pos: number }
  | { type: 'yank'; start: number; end: number }
  | { type: 'paste'; text: string }
  | { type: 'delete_line' }
  | { type: 'yank_line' }
  | { type: 'submit' }
  | { type: 'switch_mode'; mode: VimMode }
  | { type: 'undo' }
  | { type: 'passthrough' }

export type PendingOp = 'delete' | 'yank' | 'change' | null

export interface VimStateData {
  mode: VimMode
  enabled: boolean
  pendingOp: PendingOp
  repeatCount: number | null
  register: string
  visualAnchor: number
}

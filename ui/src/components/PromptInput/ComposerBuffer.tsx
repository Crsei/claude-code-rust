import React from 'react'
import { shortcutLabel } from '../../keybindings.js'
import type { KeybindingConfig } from '../../keybindings.js'
import { c } from '../../theme.js'
import { formatPasteSize, promptPlaceholder } from './utils.js'
import { shouldRenderPasteCompact, splitBufferAtCursor } from './prompt-state.js'

/**
 * Visible composer row — the ⟨before⟩⟨cursor⟩⟨after⟩ triplet, the
 * paste-compact badge, and the transcript-mode hint. Extracted from
 * `InputPrompt.tsx` so each rendering path can evolve in one place.
 *
 * Mirrors the sample tree's `BaseTextInput`
 * (`ui/examples/upstream-patterns/src/components/BaseTextInput.tsx`)
 * at the visual-only layer; the keyboard handler stays in the parent
 * composer so it can dispatch on app-level shortcuts.
 */

type Props = {
  text: string
  cursorPos: number
  isActive: boolean
  isReadOnly: boolean
  isBusy: boolean
  isPasted: boolean
  keybindingConfig: KeybindingConfig | null
}

export function ComposerBuffer({
  text,
  cursorPos,
  isActive,
  isReadOnly,
  isBusy,
  isPasted,
  keybindingConfig,
}: Props) {
  const pasteCompact = shouldRenderPasteCompact(isPasted, text.length)

  if (isReadOnly) {
    if (text.length > 0) {
      return <text fg={c.dim}>{pasteCompact ? formatPasteSize(text) : text}</text>
    }
    return (
      <text fg={c.dim}>
        Transcript mode. {shortcutLabel('app:toggleTranscript', { context: 'Global', config: keybindingConfig })} prompt.{' '}
        {shortcutLabel('transcript:exit', { context: 'Transcript', config: keybindingConfig })} exit.
      </text>
    )
  }

  if (pasteCompact) {
    return <text fg={c.warningBright}>{formatPasteSize(text)}</text>
  }

  if (text.length === 0) {
    return (
      <text>
        <span fg={c.bg} bg={isActive ? c.text : c.dim}> </span>
        <span fg="#45475A">{promptPlaceholder(isBusy)}</span>
      </text>
    )
  }

  const { before, cursorChar, after } = splitBufferAtCursor(text, cursorPos)
  return (
    <text fg={isBusy ? c.dim : undefined}>
      {before}
      <span fg={c.bg} bg={isActive ? c.text : c.dim}>{cursorChar}</span>
      {after}
    </text>
  )
}

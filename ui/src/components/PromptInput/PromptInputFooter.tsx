import React from 'react'
import { c } from '../../theme.js'
import { shortcutLabel, type KeybindingConfig } from '../../keybindings.js'

/**
 * Compact inline footer rendered directly under the composer.
 *
 * Complements (does NOT duplicate) the top `StatusLine`:
 *   - StatusLine surfaces persistent session telemetry: cwd, model,
 *     tokens, cost, agent/team/MCP/LSP counts.
 *   - This footer surfaces *composer-local* signals: vim mode hint,
 *     busy tag, queued count, and the "press ? for help" affordance.
 *
 * Mirrors the spirit of upstream `PromptInputFooter` +
 * `PromptInputFooterLeftSide`
 * (`ui/examples/upstream-patterns/src/components/PromptInput/PromptInputFooter*.tsx`)
 * but trimmed to fields we can derive from the existing store without
 * extending the IPC surface. Anything that would require backend
 * streams we don't plumb (voice, swarm, sandbox, notifications) is
 * intentionally skipped.
 */

type Props = {
  /** Set when vim is enabled — the raw mode string (`NORMAL` / `INSERT`
   *  / `VISUAL` / etc.) as surfaced by `VimState.indicator`. */
  vimMode?: string
  /** Non-empty while the backend is working; the formatted tag from
   *  `buildBusyStatus` (e.g. `"thinking 3s"`). */
  workedTag: string
  /** Number of queued prompts — shown as a compact pill when > 0 so the
   *  user sees that their Enter landed in the queue instead of
   *  vanishing. Matches the visible count rendered by
   *  `QueuedSubmissions` below. */
  queuedCount: number
  /** `true` when composer input is live (active, not readonly, not
   *  busy). Controls whether the shortcut-hint row is shown so we
   *  don't bait the user with affordances that won't respond. */
  isActive: boolean
  keybindingConfig: KeybindingConfig | null
}

export function PromptInputFooter({
  vimMode,
  workedTag,
  queuedCount,
  isActive,
  keybindingConfig,
}: Props) {
  // Gather the parts we want to show, dropping anything that's empty.
  // Rendering an empty row would reserve vertical space under the
  // composer — Yoga reserves a row even if every child is absent — so
  // we bail out entirely when there's nothing to say.
  const showVim = !!vimMode && vimMode !== 'NORMAL'
  const showBusy = workedTag.length > 0
  const showQueued = queuedCount > 0
  const showHint = isActive && !showBusy

  if (!showVim && !showBusy && !showQueued && !showHint) {
    return null
  }

  const submitLabel = shortcutLabel('chat:submit', {
    context: 'Chat',
    config: keybindingConfig,
  })
  const cancelLabel = shortcutLabel('chat:cancel', {
    context: 'Chat',
    config: keybindingConfig,
  })

  return (
    <box flexDirection="row" paddingX={2}>
      {showVim && (
        <>
          <text fg={c.warning}>-- {vimMode} --</text>
          <text fg={c.dim}> </text>
        </>
      )}
      {showBusy && (
        <>
          <text fg={c.dim}>* {workedTag}</text>
          <text fg={c.dim}> </text>
        </>
      )}
      {showQueued && (
        <>
          <text fg={c.info}>queued {queuedCount}</text>
          <text fg={c.dim}> </text>
        </>
      )}
      {showHint && (
        <text fg={c.muted}>
          {submitLabel} send * {cancelLabel} cancel
        </text>
      )}
    </box>
  )
}

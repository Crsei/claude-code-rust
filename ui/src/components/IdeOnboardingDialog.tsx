import React from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/IdeOnboardingDialog.tsx`.
 *
 * Upstream renders a welcome card that lists four IDE capabilities
 * (open files context, inline diff review, Cmd+Esc quick launch,
 * Cmd+Opt+K reference shortcut). The card is dismissed on Enter/Esc
 * and the shown-state is persisted through `saveGlobalConfig`.
 *
 * cc-rust's config writer is owned by the backend, so the dialog is a
 * pure view and the `onDone` callback's parent is responsible for
 * stamping "shown". The sample-tree helpers `hasIdeOnboardingDialogBeenShown`
 * and the terminal-based save are re-exported as pure functions so the
 * caller can plug in whatever IDE state source it has (e.g. the Lite
 * settings bridge).
 */

type InstallationStatus = {
  ideType?: string | null
  installedVersion?: string | null
}

type Props = {
  onDone: () => void
  installationStatus: InstallationStatus | null
  /** Override the detected IDE display name. Used when the backend
   *  already knows which IDE is running and we want a nicer label
   *  than the raw enum value. */
  ideDisplayName?: string
  /** Shortcut hint — macOS uses `Cmd+Option+K`, the rest of the world
   *  uses `Ctrl+Alt+K`. cc-rust has no central env helper so the
   *  caller picks the label. */
  mentionShortcut?: string
}

function isJetBrainsIde(ideType: string | null | undefined): boolean {
  if (!ideType) return false
  const lower = ideType.toLowerCase()
  return (
    lower.includes('jetbrains') ||
    lower.includes('intellij') ||
    lower.includes('webstorm') ||
    lower.includes('pycharm') ||
    lower.includes('goland')
  )
}

export function IdeOnboardingDialog({
  onDone,
  installationStatus,
  ideDisplayName,
  mentionShortcut = 'Ctrl+Alt+K',
}: Props) {
  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'escape' || name === 'return' || name === 'enter') {
      onDone()
    }
  })

  const ideType = installationStatus?.ideType
  const isJetBrains = isJetBrainsIde(ideType)
  const ideName = ideDisplayName ?? (ideType ?? 'your IDE')
  const installedVersion = installationStatus?.installedVersion
  const pluginOrExtension = isJetBrains ? 'plugin' : 'extension'
  const subtitle = installedVersion
    ? `installed ${pluginOrExtension} v${installedVersion}`
    : undefined

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.info}
      title={`✻ Welcome to Claude Code for ${ideName}`}
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      {subtitle && (
        <box marginBottom={1}>
          <text fg={c.dim} selectable>
            {subtitle}
          </text>
        </box>
      )}
      <box flexDirection="column" gap={1}>
        <text selectable>
          • Claude has context of{' '}
          <span fg={c.info}>⧉ open files</span> and{' '}
          <span fg={c.info}>⧉ selected lines</span>
        </text>
        <text selectable>
          • Review Claude Code's changes{' '}
          <span fg={c.success}>+11</span>{' '}
          <span fg={c.error}>-22</span> in the comfort of your IDE
        </text>
        <text selectable>
          • Cmd+Esc <span fg={c.dim}>for Quick Launch</span>
        </text>
        <text selectable>
          • {mentionShortcut}
          <span fg={c.dim}> to reference files or lines in your input</span>
        </text>
      </box>
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>Press Enter to continue · Esc to dismiss</span>
          </em>
        </text>
      </box>
    </box>
  )
}

/** Pure predicate counterpart of upstream
 *  `hasIdeOnboardingDialogBeenShown`. Parent supplies the per-terminal
 *  state map. */
export function hasIdeOnboardingDialogBeenShown(
  perTerminal: Record<string, boolean> | undefined,
  terminal: string,
): boolean {
  if (!perTerminal) return false
  return perTerminal[terminal] === true
}

import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import {
  extractDangerousSettings,
  formatDangerousSettingsList,
  type SettingsLike,
} from './utils.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/ManagedSettingsSecurityDialog/ManagedSettingsSecurityDialog.tsx`.
 *
 * Upstream wraps this in a `PermissionDialog` and uses Ink's `<Select>`.
 * cc-rust renders it as an absolutely-positioned warning dialog — same
 * visual weight as the LSP recommendation dialog and the permission
 * dialog — and handles keyboard selection inline. The caller provides
 * the settings payload; `onAccept` proceeds with loading managed
 * settings, `onReject` exits Claude Code.
 *
 * The two list-visual utilities are re-exported from `./utils.js` so
 * host code can check `hasDangerousSettingsChanged` without pulling in
 * this React component.
 */

type Decision = 'accept' | 'reject'

type Option = {
  value: Decision
  label: string
  hotkey: string
}

const OPTIONS: Option[] = [
  { value: 'accept', label: 'Yes, I trust these settings', hotkey: 'y' },
  { value: 'reject', label: 'No, exit Claude Code', hotkey: 'n' },
]

type Props = {
  settings: SettingsLike | null | undefined
  onAccept: () => void
  onReject: () => void
}

export function ManagedSettingsSecurityDialog({
  settings,
  onAccept,
  onReject,
}: Props) {
  const dangerous = extractDangerousSettings(settings)
  const settingsList = formatDangerousSettingsList(dangerous)

  const [selected, setSelected] = useState(0)
  const safeIndex = Math.max(0, Math.min(selected, OPTIONS.length - 1))

  const decide = (decision: Decision) => {
    if (decision === 'accept') onAccept()
    else onReject()
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input === 'y') {
      decide('accept')
      return
    }
    if (input === 'n' || name === 'escape') {
      decide('reject')
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setSelected(Math.min(OPTIONS.length - 1, safeIndex + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      decide(OPTIONS[safeIndex]!.value)
    }
  })

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title="Managed settings require approval"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="column" gap={1}>
        <text selectable>
          Your organization has configured managed settings that could allow
          execution of arbitrary code or interception of your prompts and
          responses.
        </text>

        <box flexDirection="column">
          <text fg={c.dim}>Settings requiring approval:</text>
          {settingsList.length === 0 ? (
            <box paddingLeft={2}>
              <text fg={c.dim}>
                <em>(none flagged — dialog opened manually)</em>
              </text>
            </box>
          ) : (
            settingsList.map(item => (
              <box key={item} paddingLeft={2} flexDirection="row">
                <text fg={c.dim}>· </text>
                <text selectable>{item}</text>
              </box>
            ))
          )}
        </box>

        <text selectable>
          Only accept if you trust your organization's IT administration
          and expect these settings to be configured.
        </text>

        <box flexDirection="column">
          {OPTIONS.map((opt, i) => {
            const isSelected = i === safeIndex
            return (
              <box key={opt.value} flexDirection="row">
                <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                  <strong>{` ${opt.label} `}</strong>
                </text>
                <text fg={c.dim}> ({opt.hotkey})</text>
              </box>
            )
          })}
        </box>

        <text fg={c.dim}>
          <em>Enter to confirm · Esc to exit</em>
        </text>
      </box>
    </box>
  )
}

export { extractDangerousSettings, formatDangerousSettingsList } from './utils.js'
export type { DangerousSettings, SettingsLike } from './utils.js'

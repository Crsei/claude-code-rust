import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import { isSandboxingEnabled, useSandboxAdapter } from './sandbox-adapter.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/sandbox/SandboxOverridesTab.tsx`.
 *
 * Upstream binds a two-option `<Select>` to
 * `SandboxManager.setSandboxSettings({ allowUnsandboxedCommands })`.
 * cc-rust routes the write through the adapter's `updateSettings`
 * hook and surfaces the completion message via `onComplete` — the
 * same callback the Settings shell passes down, so existing command
 * plumbing keeps working.
 */

type OverrideMode = 'open' | 'closed'

type Props = {
  onComplete: (
    result?: string,
    options?: { display?: 'skip' | 'system' },
  ) => void
}

type Option = {
  value: OverrideMode
  label: string
  hotkey: string
}

const OPTIONS: Option[] = [
  { value: 'open', label: 'Allow unsandboxed fallback', hotkey: 'a' },
  { value: 'closed', label: 'Strict sandbox mode', hotkey: 's' },
]

export function SandboxOverridesTab({ onComplete }: Props) {
  const { settings, updateSettings } = useSandboxAdapter()
  const enabled = isSandboxingEnabled(settings)
  const locked = settings.lockedByPolicy
  const currentMode: OverrideMode = settings.allowUnsandboxedCommands ? 'open' : 'closed'

  if (!enabled) {
    return (
      <box flexDirection="column" paddingY={1}>
        <text fg={c.dim}>
          Sandbox is not enabled. Enable sandbox to configure override settings.
        </text>
      </box>
    )
  }

  if (locked) {
    return (
      <box flexDirection="column" paddingY={1}>
        <text fg={c.dim}>
          Override settings are managed by a higher-priority configuration and
          cannot be changed locally.
        </text>
        <box marginTop={1}>
          <text fg={c.dim} selectable>
            Current setting:{' '}
            {currentMode === 'open' ? 'Allow unsandboxed fallback' : 'Strict sandbox mode'}
          </text>
        </box>
      </box>
    )
  }

  return (
    <OverridesSelect
      currentMode={currentMode}
      onSelect={async value => {
        await updateSettings({ allowUnsandboxedCommands: value === 'open' })
        onComplete(
          value === 'open'
            ? '\u2713 Unsandboxed fallback allowed - commands can run outside sandbox when necessary'
            : '\u2713 Strict sandbox mode - all commands must run in sandbox or be excluded via the `excludedCommands` option',
        )
      }}
      onCancel={() => onComplete(undefined, { display: 'skip' })}
    />
  )
}

function OverridesSelect({
  currentMode,
  onSelect,
  onCancel,
}: {
  currentMode: OverrideMode
  onSelect: (value: OverrideMode) => void | Promise<void>
  onCancel: () => void
}) {
  const [selected, setSelected] = useState(
    OPTIONS.findIndex(opt => opt.value === currentMode),
  )
  const safeIndex = Math.max(0, Math.min(selected, OPTIONS.length - 1))

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input) {
      const match = OPTIONS.findIndex(opt => opt.hotkey === input)
      if (match >= 0) {
        void onSelect(OPTIONS[match]!.value)
        return
      }
    }
    if (name === 'escape') {
      onCancel()
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
      void onSelect(OPTIONS[safeIndex]!.value)
    }
  })

  return (
    <box flexDirection="column" paddingY={1}>
      <box marginBottom={1}>
        <text>
          <strong>Configure Overrides:</strong>
        </text>
      </box>
      {OPTIONS.map((opt, i) => {
        const isSelected = i === safeIndex
        const isCurrent = opt.value === currentMode
        return (
          <box key={opt.value} flexDirection="row">
            <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
              <strong>{` ${opt.label} `}</strong>
            </text>
            <text fg={c.dim}> ({opt.hotkey})</text>
            {isCurrent && <text fg={c.success}> (current)</text>}
          </box>
        )
      })}
      <box flexDirection="column" marginTop={1} gap={1}>
        <text fg={c.dim} selectable>
          <strong>Allow unsandboxed fallback:</strong> When a command fails due
          to sandbox restrictions, Claude can retry with
          dangerouslyDisableSandbox to run outside the sandbox (falling back
          to default permissions).
        </text>
        <text fg={c.dim} selectable>
          <strong>Strict sandbox mode:</strong> All bash commands invoked by
          the model must run in the sandbox unless they are explicitly listed
          in excludedCommands.
        </text>
      </box>
    </box>
  )
}

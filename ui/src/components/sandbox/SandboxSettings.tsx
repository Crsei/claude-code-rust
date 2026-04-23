import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import { SandboxConfigTab } from './SandboxConfigTab.js'
import { SandboxDependenciesTab } from './SandboxDependenciesTab.js'
import { SandboxOverridesTab } from './SandboxOverridesTab.js'
import type { SandboxDependencyCheck } from './sandbox-adapter.js'
import { useSandboxAdapter } from './sandbox-adapter.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/sandbox/SandboxSettings.tsx`.
 *
 * Upstream wraps the sandbox views in a `Pane` + `Tabs` pair from the
 * upstream Ink UI kit. cc-rust has no tabs primitive; we roll our own
 * tab header (left/right or number keys switch) and render the active
 * tab body below. Tab visibility matches upstream rules:
 *   - deps have errors → only "Dependencies" tab.
 *   - deps have warnings → Mode + Dependencies + Overrides + Config.
 *   - otherwise → Mode + Overrides + Config.
 */

type SandboxMode = 'auto-allow' | 'regular' | 'disabled'

type Props = {
  onComplete: (
    result?: string,
    options?: { display?: 'skip' | 'system' },
  ) => void
  depCheck?: SandboxDependencyCheck
}

type TabKey = 'mode' | 'overrides' | 'config' | 'dependencies'

const MODE_OPTIONS: Array<{ value: SandboxMode; label: string; hotkey: string }> = [
  { value: 'auto-allow', label: 'Sandbox BashTool, with auto-allow', hotkey: 'a' },
  { value: 'regular', label: 'Sandbox BashTool, with regular permissions', hotkey: 'r' },
  { value: 'disabled', label: 'No Sandbox', hotkey: 'd' },
]

export function SandboxSettings({ onComplete, depCheck }: Props) {
  const { settings, updateSettings } = useSandboxAdapter()
  const check = depCheck ?? settings.dependencyCheck
  const hasErrors = check.errors.length > 0
  const hasWarnings = check.warnings.length > 0

  const currentMode: SandboxMode = !settings.enabled
    ? 'disabled'
    : settings.autoAllowBashIfSandboxed
      ? 'auto-allow'
      : 'regular'

  const tabs: TabKey[] = hasErrors
    ? ['dependencies']
    : hasWarnings
      ? ['mode', 'dependencies', 'overrides', 'config']
      : ['mode', 'overrides', 'config']

  const [tabIndex, setTabIndex] = useState(0)
  const safeTabIndex = Math.max(0, Math.min(tabIndex, tabs.length - 1))
  const activeTab = tabs[safeTabIndex]!

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (name === 'escape') {
      onComplete(undefined, { display: 'skip' })
      return
    }
    if (name === 'tab' || input === ']' || name === 'right') {
      setTabIndex(prev => (prev + 1) % tabs.length)
      return
    }
    if (input === '[' || name === 'left') {
      setTabIndex(prev => (prev - 1 + tabs.length) % tabs.length)
      return
    }
    // `1` / `2` / `3` / `4` jump directly to a tab by index.
    const numeric = Number(input)
    if (Number.isInteger(numeric) && numeric >= 1 && numeric <= tabs.length) {
      setTabIndex(numeric - 1)
    }
  })

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      title="Sandbox:"
      titleAlignment="left"
      paddingX={2}
      paddingY={1}
      width="100%"
    >
      <box flexDirection="row" gap={2}>
        {tabs.map((key, i) => (
          <text
            key={key}
            fg={i === safeTabIndex ? c.bg : c.dim}
            bg={i === safeTabIndex ? c.accent : undefined}
          >
            <strong>{` ${labelForTab(key)} `}</strong>
          </text>
        ))}
      </box>
      <box marginTop={1}>
        {activeTab === 'mode' && (
          <ModeTab
            currentMode={currentMode}
            showSocketWarning={
              hasWarnings && !settings.network.allowAllUnixSockets
            }
            onSelect={async value => {
              switch (value) {
                case 'auto-allow':
                  await updateSettings({ enabled: true, autoAllowBashIfSandboxed: true })
                  onComplete('\u2713 Sandbox enabled with auto-allow for bash commands')
                  break
                case 'regular':
                  await updateSettings({ enabled: true, autoAllowBashIfSandboxed: false })
                  onComplete('\u2713 Sandbox enabled with regular bash permissions')
                  break
                case 'disabled':
                  await updateSettings({ enabled: false, autoAllowBashIfSandboxed: false })
                  onComplete('\u25CB Sandbox disabled')
                  break
              }
            }}
          />
        )}
        {activeTab === 'overrides' && <SandboxOverridesTab onComplete={onComplete} />}
        {activeTab === 'config' && <SandboxConfigTab />}
        {activeTab === 'dependencies' && <SandboxDependenciesTab depCheck={check} />}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          <em>
            Tab / \u2190 \u2192 to switch tabs · 1-{tabs.length} jump · Esc to close
          </em>
        </text>
      </box>
    </box>
  )
}

function labelForTab(key: TabKey): string {
  switch (key) {
    case 'mode':
      return 'Mode'
    case 'overrides':
      return 'Overrides'
    case 'config':
      return 'Config'
    case 'dependencies':
      return 'Dependencies'
  }
}

function ModeTab({
  currentMode,
  showSocketWarning,
  onSelect,
}: {
  currentMode: SandboxMode
  showSocketWarning: boolean
  onSelect: (mode: SandboxMode) => void | Promise<void>
}) {
  const [selected, setSelected] = useState(
    MODE_OPTIONS.findIndex(opt => opt.value === currentMode),
  )
  const safeIndex = Math.max(0, Math.min(selected, MODE_OPTIONS.length - 1))

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()
    if (input) {
      const match = MODE_OPTIONS.findIndex(opt => opt.hotkey === input)
      if (match >= 0) {
        void onSelect(MODE_OPTIONS[match]!.value)
        return
      }
    }
    if (name === 'up' || input === 'k') {
      setSelected(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setSelected(Math.min(MODE_OPTIONS.length - 1, safeIndex + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      void onSelect(MODE_OPTIONS[safeIndex]!.value)
    }
  })

  return (
    <box flexDirection="column" paddingY={1}>
      {showSocketWarning && (
        <box marginBottom={1}>
          <text fg={c.warning}>
            Cannot block unix domain sockets (see Dependencies tab)
          </text>
        </box>
      )}
      <box marginBottom={1}>
        <text>
          <strong>Configure Mode:</strong>
        </text>
      </box>
      {MODE_OPTIONS.map((opt, i) => {
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
      <box flexDirection="column" marginTop={1}>
        <text fg={c.dim} selectable>
          <strong>Auto-allow mode:</strong> Commands will try to run in the
          sandbox automatically, and attempts to run outside of the sandbox
          fallback to regular permissions. Explicit ask/deny rules are always
          respected.
        </text>
      </box>
    </box>
  )
}

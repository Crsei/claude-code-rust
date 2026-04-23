import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import { Config } from './Config.js'
import { Status, type Diagnostic, type Property } from './Status.js'
import { Usage, type Utilization } from './Usage.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/Settings/Settings.tsx`.
 *
 * Upstream's shell renders a `Pane` + `Tabs` container whose children
 * are the Status / Config / Usage tabs. cc-rust has no Pane/Tabs
 * primitive; we roll a minimal tab strip that:
 *
 * - Starts on `defaultTab` (matching upstream's initial tab rule).
 * - Cycles with Tab / left / right or numeric keys 1-3.
 * - Forwards Esc to `onClose` unless a descendant handles it first
 *   (e.g. `Config` clears the search query on Esc).
 *
 * The Status / Usage tabs are pure view components from this folder,
 * so the shell stays tiny. `context` is kept as an opaque pass-through
 * so callers that already have a `LocalJSXCommandContext` from
 * upstream don't have to rewrap.
 */

type TabKey = 'status' | 'config' | 'usage'

type Props = {
  defaultTab?: 'Status' | 'Config' | 'Usage'
  onClose: (result?: string) => void
  /** Properties passed into the Status tab. */
  statusExtraSections?: Property[][]
  statusDiagnostics?: Diagnostic[]
  statusDiagnosticsLoading?: boolean
  /** Loader for the Usage tab. */
  loadUtilization?: () => Promise<Utilization | null>
  /** Subscriber tier — drives which Usage bars render. */
  subscriptionType?: 'pro' | 'max' | 'team' | 'free' | null
}

const TAB_ORDER: TabKey[] = ['status', 'config', 'usage']

function tabKeyFromName(name?: Props['defaultTab']): TabKey {
  switch (name) {
    case 'Config':
      return 'config'
    case 'Usage':
      return 'usage'
    default:
      return 'status'
  }
}

export function Settings({
  defaultTab = 'Status',
  onClose,
  statusExtraSections,
  statusDiagnostics,
  statusDiagnosticsLoading,
  loadUtilization,
  subscriptionType,
}: Props) {
  const [active, setActive] = useState<TabKey>(tabKeyFromName(defaultTab))

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (name === 'tab' || name === 'right' || input === ']') {
      setActive(prev => {
        const i = TAB_ORDER.indexOf(prev)
        return TAB_ORDER[(i + 1) % TAB_ORDER.length]!
      })
      return
    }
    if (name === 'left' || input === '[') {
      setActive(prev => {
        const i = TAB_ORDER.indexOf(prev)
        return TAB_ORDER[(i - 1 + TAB_ORDER.length) % TAB_ORDER.length]!
      })
      return
    }
    if (input === '1') setActive('status')
    else if (input === '2') setActive('config')
    else if (input === '3') setActive('usage')
  })

  return (
    <box
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor={c.accent}
      title="Settings"
      titleAlignment="left"
      paddingX={2}
      paddingY={1}
      width="100%"
    >
      <box flexDirection="row" gap={2}>
        {TAB_ORDER.map(key => (
          <text
            key={key}
            fg={key === active ? c.bg : c.dim}
            bg={key === active ? c.accent : undefined}
          >
            <strong>{` ${labelForTab(key)} `}</strong>
          </text>
        ))}
      </box>
      <box marginTop={1}>
        {active === 'status' && (
          <Status
            extraSections={statusExtraSections}
            diagnostics={statusDiagnostics}
            diagnosticsLoading={statusDiagnosticsLoading}
          />
        )}
        {active === 'config' && (
          <Config onClose={onClose} />
        )}
        {active === 'usage' && (
          <Usage
            loadUtilization={loadUtilization}
            subscriptionType={subscriptionType}
          />
        )}
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          <em>
            Tab / ←/→ to switch · 1-3 jump · Esc closes
          </em>
        </text>
      </box>
    </box>
  )
}

function labelForTab(key: TabKey): string {
  switch (key) {
    case 'status':
      return 'Status'
    case 'config':
      return 'Config'
    case 'usage':
      return 'Usage'
  }
}

export { Config } from './Config.js'
export { Status, buildDefaultDiagnostics } from './Status.js'
export type { Diagnostic, Property } from './Status.js'
export { Usage } from './Usage.js'
export type { RateLimit, Utilization, ExtraUsage } from './Usage.js'

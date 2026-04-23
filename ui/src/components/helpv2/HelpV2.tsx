import React, { useMemo, useState } from 'react'
import { useKeyboard, useTerminalDimensions } from '@opentui/react'
import { c } from '../../theme.js'
import { shortcutLabel } from '../../keybindings.js'
import { useAppState } from '../../store/app-store.js'
import { Commands, type HelpCommandEntry } from './Commands.js'
import { General } from './General.js'

/**
 * Tabbed help overlay (`/help`).
 *
 * OpenTUI-native port of the upstream `helpv2/HelpV2`
 * (`ui/examples/upstream-patterns/src/components/helpv2/HelpV2.tsx`).
 * Upstream pulled `Pane`, `Tabs`, `Tab`, analytics helpers, and the
 * legacy `commands` registry through Ink. The Lite port keeps the same
 * tab taxonomy (general / commands / custom-commands / ant-only) and
 * the "press Esc to dismiss / ctrl+c twice to exit" gesture, but expects
 * the caller to hand in a flat `HelpCommandEntry` list \u2014 the Rust
 * backend owns the command registry.
 */

type TabId = 'general' | 'commands' | 'custom-commands' | 'ant-only'

type HelpTab = {
  id: TabId
  title: string
  emptyMessage?: string
  commands?: HelpCommandEntry[]
  heading?: string
}

type Props = {
  onClose: () => void
  /** Built-in commands (shown under the "commands" tab). */
  builtinCommands: HelpCommandEntry[]
  /** Project / user / plugin commands (shown under "custom-commands"). */
  customCommands?: HelpCommandEntry[]
  /** ANT-only commands (only rendered when `showAntOnly` is true). */
  antOnlyCommands?: HelpCommandEntry[]
  /** When true, expose the `[ant-only]` tab. Defaults to `false`. */
  showAntOnly?: boolean
  /** Version string shown in the frame title. */
  version?: string
}

const DOCS_URL = 'https://docs.claude.com/en/docs/claude-code/overview'

export function HelpV2({
  onClose,
  builtinCommands,
  customCommands = [],
  antOnlyCommands = [],
  showAntOnly = false,
  version,
}: Props) {
  const { width: columns, height: rows } = useTerminalDimensions()
  const maxHeight = Math.max(20, Math.floor(rows / 2))
  const { keybindingConfig } = useAppState()

  const dismissShortcut =
    shortcutLabel('help:dismiss', { context: 'Help', config: keybindingConfig ?? null }) ||
    'esc'

  const tabs = useMemo<HelpTab[]>(() => {
    const list: HelpTab[] = [
      { id: 'general', title: 'general' },
      {
        id: 'commands',
        title: 'commands',
        heading: 'Browse default commands:',
        commands: builtinCommands,
      },
      {
        id: 'custom-commands',
        title: 'custom-commands',
        heading: 'Browse custom commands:',
        commands: customCommands,
        emptyMessage: 'No custom commands found',
      },
    ]
    if (showAntOnly && antOnlyCommands.length > 0) {
      list.push({
        id: 'ant-only',
        title: '[ant-only]',
        heading: 'Browse ant-only commands:',
        commands: antOnlyCommands,
      })
    }
    return list
  }, [antOnlyCommands, builtinCommands, customCommands, showAntOnly])

  const [activeIndex, setActiveIndex] = useState(0)
  const [headerFocused, setHeaderFocused] = useState(true)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    if (name === 'escape') {
      onClose()
      return
    }
    if (!headerFocused) return
    if (name === 'tab' || name === 'right') {
      setActiveIndex(prev => (prev + 1) % tabs.length)
      return
    }
    if (name === 'left') {
      setActiveIndex(prev => (prev - 1 + tabs.length) % tabs.length)
      return
    }
    if (name === 'down') {
      setHeaderFocused(false)
    }
  })

  const activeTab = tabs[activeIndex]!
  const title = version ? `Claude Code v${version}` : '/help'

  return (
    <box
      position="absolute"
      top={2}
      left={2}
      right={2}
      bottom={2}
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor={c.info}
      title={title}
      titleAlignment="left"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="row" gap={1}>
        {tabs.map((tab, i) => {
          const isActive = i === activeIndex
          return (
            <text
              key={tab.id}
              fg={isActive ? c.bg : c.dim}
              bg={isActive ? c.info : undefined}
            >
              <strong>{` ${tab.title} `}</strong>
            </text>
          )
        })}
      </box>
      <box marginTop={1} flexGrow={1} flexDirection="column">
        {activeTab.id === 'general' ? (
          <General />
        ) : (
          <Commands
            commands={activeTab.commands ?? []}
            maxHeight={maxHeight}
            columns={columns}
            title={activeTab.heading ?? ''}
            emptyMessage={activeTab.emptyMessage}
            onCancel={onClose}
            isDisabled={headerFocused}
            onUpFromFirstItem={() => setHeaderFocused(true)}
          />
        )}
      </box>
      <box marginTop={1} flexDirection="column">
        <text>
          For more help: <span fg={c.info}>{DOCS_URL}</span>
        </text>
        <text>
          <em>
            <span fg={c.dim}>{`${dismissShortcut} to cancel`}</span>
          </em>
        </text>
      </box>
    </box>
  )
}

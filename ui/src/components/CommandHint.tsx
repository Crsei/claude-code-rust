import React from 'react'
import { shortcutLabel } from '../keybindings.js'
import { c } from '../theme.js'
import { KIND_COLORS, type CommandDef } from '../commands.js'
import { useAppState } from '../store/app-store.js'

const MAX_VISIBLE = 10

interface Props {
  matches: CommandDef[]
  selectedIndex: number
  partial: string
  subOptions?: string[]
  subSelectedIndex?: number
}

export function CommandHint({
  matches,
  selectedIndex,
  partial,
  subOptions,
  subSelectedIndex,
}: Props) {
  const { keybindingConfig } = useAppState()

  if (subOptions && subOptions.length > 0) {
    const { start, end } = visibleWindow(subOptions.length, subSelectedIndex ?? 0)
    const visibleOptions = subOptions.slice(start, end)
    const countLabel = subOptions.length > MAX_VISIBLE
      ? `${start + 1}-${end} of ${subOptions.length} options`
      : `${subOptions.length} options`

    return (
      <box flexDirection="column" paddingLeft={3} flexShrink={0}>
        {visibleOptions.map((option, offset) => {
          const index = start + offset
          const selected = index === (subSelectedIndex ?? 0)
          return (
            <box key={option} flexDirection="row" gap={1}>
              <text fg={selected ? c.accent : c.info} bg={selected ? '#1a1a2e' : undefined}>
                {selected ? '>' : ' '} <strong>{option}</strong>
              </text>
            </box>
          )
        })}
        <box paddingLeft={1}>
          <text fg={c.muted}>
            {countLabel}  Up/Down navigate  {shortcutLabel('select:accept', { context: 'Select', config: keybindingConfig })} confirm  {shortcutLabel('select:cancel', { context: 'Select', config: keybindingConfig })} cancel
          </text>
        </box>
      </box>
    )
  }

  if (matches.length === 0) {
    return (
      <box paddingLeft={3}>
        <text fg={c.dim}>
          <em>No matching commands</em>
        </text>
      </box>
    )
  }

  const { start, end } = visibleWindow(matches.length, selectedIndex)
  const list = matches.slice(start, end).map((cmd, offset) => {
    const index = start + offset
    const selected = index === selectedIndex
    const highlight = highlightMatch(cmd.name, partial)
    const color = KIND_COLORS[cmd.kind]
    const usage = usageForCommand(cmd)

    return (
      <box key={cmd.name} id={`cmd-${index}`} flexDirection="row" gap={1}>
        <text fg={color}>*</text>
        <text fg={selected ? c.accent : c.info} bg={selected ? '#1a1a2e' : undefined}>
          {selected ? '>' : ' '}
          <strong>
            /{highlight.before}
            <span fg={c.textBright}>{highlight.match}</span>
            {highlight.after}
          </strong>
        </text>
        {cmd.aliases.length > 0 && (
          <text fg={c.muted}>({cmd.aliases.map(alias => `/${alias}`).join(', ')})</text>
        )}
        {usage && <text fg={c.muted}>{usage}</text>}
        <text fg={c.dim}>{cmd.description}</text>
      </box>
    )
  })

  const countLabel = matches.length > MAX_VISIBLE
    ? `${start + 1}-${end} of ${matches.length} commands`
    : `${matches.length} commands`

  return (
    <box flexDirection="column" paddingLeft={3} flexShrink={0}>
      {list}
      <box paddingLeft={1}>
        <text fg={c.muted}>
          {countLabel}  {shortcutLabel('autocomplete:accept', { context: 'Autocomplete', config: keybindingConfig })} complete  Up/Down navigate  {shortcutLabel('select:accept', { context: 'Select', config: keybindingConfig })} run
        </text>
      </box>
    </box>
  )
}

export function visibleWindow(
  total: number,
  selectedIndex: number,
  maxVisible = MAX_VISIBLE,
): { start: number; end: number } {
  if (total <= 0 || maxVisible <= 0) {
    return { start: 0, end: 0 }
  }

  const size = Math.min(total, maxVisible)
  const selected = Math.min(Math.max(selectedIndex, 0), total - 1)
  const centeredStart = selected - Math.floor(size / 2)
  const start = Math.min(Math.max(centeredStart, 0), total - size)
  return { start, end: start + size }
}

function usageForCommand(cmd: CommandDef): string | undefined {
  if (cmd.usage?.trim()) {
    return cmd.usage.trim()
  }
  if (cmd.kind === 'select' && cmd.options?.length) {
    return cmd.options.join('|')
  }
  if (cmd.kind === 'input' && cmd.argHint?.trim()) {
    return cmd.argHint.trim()
  }
  return undefined
}

function highlightMatch(
  name: string,
  partial: string,
): { before: string; match: string; after: string } {
  if (!partial) return { before: '', match: '', after: name }
  const index = name.toLowerCase().indexOf(partial.toLowerCase())
  if (index === -1) return { before: name, match: '', after: '' }
  return {
    before: name.slice(0, index),
    match: name.slice(index, index + partial.length),
    after: name.slice(index + partial.length),
  }
}

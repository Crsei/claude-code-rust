import React, { useEffect, useRef } from 'react'
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
  const scrollRef = useRef<any>(null)
  const { keybindingConfig } = useAppState()

  useEffect(() => {
    scrollRef.current?.scrollChildIntoView?.(`cmd-${selectedIndex}`)
  }, [selectedIndex])

  if (subOptions && subOptions.length > 0) {
    return (
      <box flexDirection="column" paddingLeft={3}>
        {subOptions.map((option, index) => {
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
            Up/Down navigate  {shortcutLabel('select:accept', { context: 'Select', config: keybindingConfig })} confirm  {shortcutLabel('select:cancel', { context: 'Select', config: keybindingConfig })} cancel
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

  const list = matches.map((cmd, index) => {
    const selected = index === selectedIndex
    const highlight = highlightMatch(cmd.name, partial)
    const color = KIND_COLORS[cmd.kind]

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
        {cmd.kind === 'select' && cmd.options && (
          <text fg={c.muted}>[{cmd.options.join('|')}]</text>
        )}
        {cmd.kind === 'input' && cmd.argHint && (
          <text fg={c.muted}>{cmd.argHint}</text>
        )}
        <text fg={c.dim}>{cmd.description}</text>
      </box>
    )
  })

  const needsScroll = matches.length > MAX_VISIBLE

  return (
    <box flexDirection="column" paddingLeft={3}>
      {needsScroll ? (
        <scrollbox ref={scrollRef} height={MAX_VISIBLE} focused={false}>
          {list}
        </scrollbox>
      ) : list}
      <box paddingLeft={1}>
        <text fg={c.muted}>
          {matches.length} commands  {shortcutLabel('autocomplete:accept', { context: 'Autocomplete', config: keybindingConfig })} complete  Up/Down navigate  {shortcutLabel('select:accept', { context: 'Select', config: keybindingConfig })} run
        </text>
      </box>
    </box>
  )
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

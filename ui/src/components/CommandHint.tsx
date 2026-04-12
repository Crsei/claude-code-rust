import React, { useEffect, useRef } from 'react'
import { c } from '../theme.js'
import { KIND_COLORS, type CommandDef } from '../commands.js'

const MAX_VISIBLE = 10

interface Props {
  matches: CommandDef[]
  selectedIndex: number
  partial: string
  /** When set, we're in sub-option selection mode for this command */
  subOptions?: string[]
  subSelectedIndex?: number
}

export function CommandHint({ matches, selectedIndex, partial, subOptions, subSelectedIndex }: Props) {
  const scrollRef = useRef<any>(null)

  useEffect(() => {
    scrollRef.current?.scrollChildIntoView?.(`cmd-${selectedIndex}`)
  }, [selectedIndex])

  // Sub-option mode: show options for a select-kind command
  if (subOptions && subOptions.length > 0) {
    return (
      <box flexDirection="column" paddingLeft={3}>
        {subOptions.map((opt, i) => {
          const selected = i === (subSelectedIndex ?? 0)
          return (
            <box key={opt} flexDirection="row" gap={1}>
              <text fg={selected ? c.accent : c.info} bg={selected ? '#1a1a2e' : undefined}>
                {selected ? '▸' : ' '} <strong>{opt}</strong>
              </text>
            </box>
          )
        })}
        <box paddingLeft={1}>
          <text fg={c.muted}>
            <span fg={c.dim}>↑↓</span> navigate  <span fg={c.dim}>Enter</span> confirm  <span fg={c.dim}>Esc</span> cancel
          </text>
        </box>
      </box>
    )
  }

  // No matches
  if (matches.length === 0) {
    return (
      <box paddingLeft={3}>
        <text fg={c.dim}><em>No matching commands</em></text>
      </box>
    )
  }

  // Command list mode
  const list = matches.map((cmd, i) => {
    const selected = i === selectedIndex
    const hl = highlightMatch(cmd.name, partial)
    const color = KIND_COLORS[cmd.kind]

    return (
      <box key={cmd.name} id={`cmd-${i}`} flexDirection="row" gap={1}>
        <text fg={color}>●</text>
        <text fg={selected ? c.accent : c.info} bg={selected ? '#1a1a2e' : undefined}>
          {selected ? '▸' : ' '}
          <strong>/{hl.before}<span fg={c.textBright}>{hl.match}</span>{hl.after}</strong>
        </text>
        {cmd.aliases.length > 0 && (
          <text fg={c.muted}>({cmd.aliases.map(a => `/${a}`).join(', ')})</text>
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
          {matches.length} commands  <span fg={c.dim}>Tab</span> complete  <span fg={c.dim}>↑↓</span> navigate  <span fg={c.dim}>Enter</span> run
        </text>
      </box>
    </box>
  )
}

function highlightMatch(name: string, partial: string): { before: string; match: string; after: string } {
  if (!partial) return { before: '', match: '', after: name }
  const idx = name.toLowerCase().indexOf(partial.toLowerCase())
  if (idx === -1) return { before: name, match: '', after: '' }
  return {
    before: name.slice(0, idx),
    match: name.slice(idx, idx + partial.length),
    after: name.slice(idx + partial.length),
  }
}

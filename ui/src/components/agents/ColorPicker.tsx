import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../../theme.js'

/**
 * Lite-native port of upstream's
 * `ui/examples/upstream-patterns/src/components/agents/ColorPicker.tsx`.
 *
 * Upstream reaches for the `AGENT_COLORS` constant out of the builtin
 * tools package; the Lite frontend keeps a local mirror so the picker
 * compiles without the full tool registry. Selecting `automatic`
 * returns `undefined` so the backend falls back to its automatic
 * colour map.
 */

export type AgentColorName =
  | 'red'
  | 'orange'
  | 'yellow'
  | 'green'
  | 'blue'
  | 'purple'
  | 'pink'
  | 'cyan'

type ColorOption = AgentColorName | 'automatic'

const AGENT_COLORS: AgentColorName[] = [
  'red',
  'orange',
  'yellow',
  'green',
  'blue',
  'purple',
  'pink',
  'cyan',
]

const COLOR_OPTIONS: ColorOption[] = ['automatic', ...AGENT_COLORS]

const AGENT_COLOR_HEX: Record<AgentColorName, string> = {
  red: '#F38BA8',
  orange: '#FAB387',
  yellow: '#F9E2AF',
  green: '#A6E3A1',
  blue: '#89B4FA',
  purple: '#CBA6F7',
  pink: '#F5C2E7',
  cyan: '#94E2D5',
}

function capitalize(s: string): string {
  return s.length === 0 ? s : s[0]!.toUpperCase() + s.slice(1)
}

type Props = {
  agentName: string
  currentColor?: AgentColorName | 'automatic'
  onConfirm: (color: AgentColorName | undefined) => void
  onCancel?: () => void
}

export function ColorPicker({ agentName, currentColor = 'automatic', onConfirm, onCancel }: Props) {
  const [selected, setSelected] = useState(
    Math.max(0, COLOR_OPTIONS.findIndex(opt => opt === currentColor)),
  )

  useKeyboard((event: KeyEvent) => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const key = (seq ?? name ?? '').toLowerCase()

    if (name === 'escape') {
      onCancel?.()
      return
    }
    if (name === 'up' || key === 'k') {
      setSelected(prev => (prev > 0 ? prev - 1 : COLOR_OPTIONS.length - 1))
      return
    }
    if (name === 'down' || key === 'j') {
      setSelected(prev => (prev < COLOR_OPTIONS.length - 1 ? prev + 1 : 0))
      return
    }
    if (name === 'return' || name === 'enter') {
      const choice = COLOR_OPTIONS[selected]
      onConfirm(choice === 'automatic' ? undefined : choice)
    }
  })

  const previewColor = COLOR_OPTIONS[selected]

  return (
    <box flexDirection="column" gap={1}>
      <box flexDirection="column">
        {COLOR_OPTIONS.map((option, i) => {
          const isSelected = i === selected
          return (
            <box key={option} flexDirection="row" gap={1}>
              <text fg={isSelected ? c.accent : c.dim}>
                {isSelected ? '\u276F' : ' '}
              </text>
              {option === 'automatic' ? (
                isSelected ? (
                  <strong><text>Automatic color</text></strong>
                ) : (
                  <text>Automatic color</text>
                )
              ) : (
                <box flexDirection="row" gap={1}>
                  <text bg={AGENT_COLOR_HEX[option]} fg={c.bg}>
                    {'  '}
                  </text>
                  {isSelected ? (
                    <strong><text>{capitalize(option)}</text></strong>
                  ) : (
                    <text>{capitalize(option)}</text>
                  )}
                </box>
              )}
            </box>
          )
        })}
      </box>

      <box marginTop={1} flexDirection="row" gap={1}>
        <text>Preview:</text>
        {previewColor === undefined || previewColor === 'automatic' ? (
          <strong><text bg={c.textBright} fg={c.bg}>{' @'}{agentName}{' '}</text></strong>
        ) : (
          <strong>
            <text bg={AGENT_COLOR_HEX[previewColor as AgentColorName]} fg={c.bg}>
              {' @'}{agentName}{' '}
            </text>
          </strong>
        )}
      </box>
    </box>
  )
}

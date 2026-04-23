import React from 'react'
import { c } from '../theme.js'
import { formatNumber } from '../utils.js'

/**
 * Single-row agent progress indicator. OpenTUI-native port of the
 * upstream `AgentProgressLine`
 * (`ui/examples/upstream-patterns/src/components/AgentProgressLine.tsx`).
 *
 * Rendered underneath an active agent to show:
 *   - tree-style prefix (`├─`/`└─`) chaining to siblings,
 *   - optional `agentType` / `description` chips,
 *   - tool-use counter + token total,
 *   - a second line with the latest tool activity or a completion state.
 */

type Props = {
  agentType: string
  description?: string
  name?: string
  descriptionColor?: string
  taskDescription?: string
  toolUseCount: number
  tokens: number | null
  color?: string
  isLast: boolean
  isResolved: boolean
  isError: boolean
  isAsync?: boolean
  shouldAnimate?: boolean
  lastToolInfo?: string | null
  hideType?: boolean
}

export function AgentProgressLine({
  agentType,
  description,
  name,
  descriptionColor,
  taskDescription,
  toolUseCount,
  tokens,
  color,
  isLast,
  isResolved,
  isError,
  isAsync = false,
  lastToolInfo,
  hideType = false,
}: Props) {
  const treeChar = isLast ? '\u2514\u2500' : '\u251C\u2500'
  const isBackgrounded = isAsync && isResolved

  const statusText = !isResolved
    ? lastToolInfo || 'Initializing\u2026'
    : isBackgrounded
      ? taskDescription ?? 'Running in the background'
      : isError
        ? 'Errored'
        : 'Done'

  const headerBgColor = color ?? undefined
  const descBgColor = descriptionColor ?? undefined
  const labelFg = headerBgColor ? c.bg : undefined
  const descFg = descBgColor ? c.bg : undefined

  return (
    <box flexDirection="column">
      <box paddingLeft={3} flexDirection="row">
        <text fg={c.dim}>{treeChar} </text>
        <text fg={isResolved ? undefined : c.dim}>
          {hideType ? (
            <>
              <strong>{name ?? description ?? agentType}</strong>
              {name && description && (
                <span fg={c.dim}>{`: ${description}`}</span>
              )}
            </>
          ) : (
            <>
              <strong>
                <span fg={labelFg} bg={headerBgColor}>{agentType}</span>
              </strong>
              {description && (
                <>
                  {' ('}
                  <span fg={descFg} bg={descBgColor}>{description}</span>
                  {')'}
                </>
              )}
            </>
          )}
          {!isBackgrounded && (
            <>
              {' \u00B7 '}
              {toolUseCount} tool {toolUseCount === 1 ? 'use' : 'uses'}
              {tokens !== null && tokens !== undefined && (
                <> {' \u00B7 '} {formatNumber(tokens)} tokens</>
              )}
            </>
          )}
        </text>
      </box>
      {!isBackgrounded && (
        <box paddingLeft={3} flexDirection="row">
          <text fg={c.dim}>{isLast ? '   \u23BF  ' : '\u2502  \u23BF  '}</text>
          <text fg={c.dim}>{statusText}</text>
        </box>
      )}
    </box>
  )
}

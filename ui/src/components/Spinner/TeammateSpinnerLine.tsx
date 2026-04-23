import React, { useRef } from 'react'
import { c } from '../../theme.js'
import { TEAMMATE_SELECT_HINT } from './teammateSelectHint.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/TeammateSpinnerLine.tsx`.
 *
 * A single row in the teammate spinner tree. Upstream leans on a number
 * of Ink-only helpers (`useTerminalSize`, `useElapsedTime`,
 * `useAppState`, `summarizeRecentActivities`, `formatDuration`,
 * `figures`) — all of which the Rust port either surfaces through
 * injected props or replaces with simple unicode literals.
 *
 * The resulting component stays a pure leaf: given a `TeammateLineProps`
 * snapshot it renders the familiar `\u251c\u2500 @name: activity\u2026` row, with
 * responsive hiding of stats / hints as `columns` shrinks.
 */

const POINTER = '\u203A'
const ELLIPSIS = '\u2026'

export type TeammateLineProps = {
  /** Display name without the leading `@`. */
  agentName: string
  /** Agent colour — hex. */
  nameColor: string
  /** Terminal width in columns. */
  columns: number
  isLast: boolean
  isSelected?: boolean
  isForegrounded?: boolean
  /** When true, every teammate is idle and the freeze-duration text kicks in. */
  allIdle?: boolean
  /** Current activity line — `undefined` when the teammate is idle. */
  activity?: string
  /** Pre-formatted idle / past-tense duration (upstream `formatDuration`). */
  displayDuration?: string
  /** Past-tense verb shown after all teammates go idle. */
  pastTenseVerb?: string
  /** Running-tool counter. */
  toolUseCount: number
  /** Token counter. */
  tokenCount: number
  /** Stopping / awaiting-approval status flags. */
  shutdownRequested?: boolean
  awaitingPlanApproval?: boolean
  isIdle?: boolean
}

export function TeammateSpinnerLine({
  agentName,
  nameColor,
  columns,
  isLast,
  isSelected,
  isForegrounded,
  allIdle,
  activity,
  displayDuration,
  pastTenseVerb,
  toolUseCount,
  tokenCount,
  shutdownRequested,
  awaitingPlanApproval,
  isIdle,
}: TeammateLineProps): React.ReactElement {
  const isHighlighted = isSelected || isForegrounded
  const treeChar = isHighlighted
    ? isLast ? '\u2558\u2550' : '\u255e\u2550'
    : isLast ? '\u2514\u2500' : '\u251c\u2500'

  const basePrefix = 8
  const fullAgentName = `@${agentName}`
  const fullNameWidth = fullAgentName.length
  const statsText = ` \u00b7 ${toolUseCount} tool ${toolUseCount === 1 ? 'use' : 'uses'} \u00b7 ${tokenCount} tokens`
  const statsWidth = statsText.length
  const selectHintText = ` \u00b7 ${TEAMMATE_SELECT_HINT}`
  const selectHintWidth = selectHintText.length
  const viewHintText = ' \u00b7 enter to view'
  const viewHintWidth = viewHintText.length

  const minActivityWidth = 25
  const spaceWithFullName = columns - basePrefix - fullNameWidth - 2
  const showName = columns >= 60 && spaceWithFullName >= minActivityWidth
  const nameWidth = showName ? fullNameWidth + 2 : 0
  const availableForActivity = columns - basePrefix - nameWidth

  const showViewHint =
    isSelected === true &&
    !isForegrounded &&
    availableForActivity >
      viewHintWidth + statsWidth + minActivityWidth + 5
  const showSelectHint =
    isHighlighted === true &&
    availableForActivity >
      selectHintWidth +
        (showViewHint ? viewHintWidth : 0) +
        statsWidth +
        minActivityWidth +
        5
  const showStats = availableForActivity > statsWidth + minActivityWidth + 5

  const frozenDurationRef = useRef<string | null>(null)
  if (!allIdle && frozenDurationRef.current !== null) {
    frozenDurationRef.current = null
  }
  if (allIdle && frozenDurationRef.current === null && displayDuration) {
    frozenDurationRef.current = displayDuration
  }
  const shownDuration = allIdle
    ? (frozenDurationRef.current ?? displayDuration ?? '')
    : (displayDuration ?? '')

  const renderStatus = (): React.ReactNode => {
    if (shutdownRequested) return <text fg={c.dim}>[stopping]</text>
    if (awaitingPlanApproval) {
      return <text fg={c.warning}>[awaiting approval]</text>
    }
    if (isIdle) {
      if (allIdle) {
        return (
          <text fg={c.dim}>
            {pastTenseVerb ?? 'idle'} for {shownDuration}
          </text>
        )
      }
      return <text fg={c.dim}>Idle for {shownDuration}</text>
    }
    if (isHighlighted) return null
    const line = activity ?? ''
    const withEllipsis = line.endsWith(ELLIPSIS) ? line : `${line}${ELLIPSIS}`
    return <text fg={c.dim}>{withEllipsis}</text>
  }

  return (
    <box flexDirection="column" paddingLeft={3}>
      <box flexDirection="row">
        <text fg={isSelected ? c.accent : undefined}>
          {isSelected ? <strong>{POINTER}</strong> : ' '}
        </text>
        <text fg={isSelected ? c.text : c.dim}>{treeChar} </text>
        {showName ? (
          <text fg={isSelected ? c.accent : nameColor}>@{agentName}</text>
        ) : null}
        {showName ? (
          <text fg={isSelected ? c.text : c.dim}>: </text>
        ) : null}
        {renderStatus()}
        {showStats ? (
          <text fg={c.dim}>
            {' '}
            \u00b7 {toolUseCount} tool {toolUseCount === 1 ? 'use' : 'uses'} \u00b7{' '}
            {tokenCount} tokens
          </text>
        ) : null}
        {showSelectHint ? (
          <text fg={c.dim}> \u00b7 {TEAMMATE_SELECT_HINT}</text>
        ) : null}
        {showViewHint ? <text fg={c.dim}> \u00b7 enter to view</text> : null}
      </box>
    </box>
  )
}

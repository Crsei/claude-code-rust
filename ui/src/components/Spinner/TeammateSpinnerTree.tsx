import React from 'react'
import { c } from '../../theme.js'
import { TEAMMATE_SELECT_HINT } from './teammateSelectHint.js'
import { TeammateSpinnerLine, type TeammateLineProps } from './TeammateSpinnerLine.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/Spinner/TeammateSpinnerTree.tsx`.
 *
 * Renders the `team-lead + teammates + hide` row group. Upstream pulls
 * teammate state out of `useAppState` / `getRunningTeammatesSorted`; the
 * Rust port passes `teammates` (as `TeammateLineProps[]` minus the tree
 * bookkeeping) straight through so this file stays a presentation
 * component.
 */

const POINTER = '\u203A'

type TreeTeammate = Omit<TeammateLineProps, 'isLast' | 'isSelected' | 'isForegrounded' | 'allIdle'>

type Props = {
  teammates: (TreeTeammate & { id: string })[]
  /** Index of the currently selected teammate (-1 = leader, n = teammates[n], n = teammates.length = hide row). */
  selectedIndex?: number
  isInSelectionMode?: boolean
  allIdle?: boolean
  /** Teammate id currently foregrounded (rendered as highlighted). */
  foregroundedId?: string | null
  /** Leader row inputs. */
  leaderVerb?: string
  leaderTokenCount?: number
  leaderIdleText?: string
}

export function TeammateSpinnerTree({
  teammates,
  selectedIndex,
  isInSelectionMode,
  allIdle,
  foregroundedId,
  leaderVerb,
  leaderTokenCount,
  leaderIdleText,
}: Props): React.ReactElement | null {
  if (teammates.length === 0) return null

  const isLeaderForegrounded = foregroundedId == null
  const isLeaderSelected = isInSelectionMode && selectedIndex === -1
  const isLeaderHighlighted = isLeaderForegrounded || isLeaderSelected

  const isHideSelected =
    isInSelectionMode === true && selectedIndex === teammates.length

  return (
    <box flexDirection="column" marginTop={1}>
      <box flexDirection="row" paddingLeft={3}>
        <text fg={isLeaderSelected ? c.accent : undefined}>
          {isLeaderHighlighted ? <strong>{isLeaderSelected ? POINTER : ' '}</strong> : ' '}
        </text>
        <text fg={isLeaderHighlighted ? c.text : c.dim}>
          {isLeaderHighlighted
            ? <strong>{'\u2552\u2550 '}</strong>
            : '\u250C\u2500 '}
        </text>
        <text fg={isLeaderSelected ? c.accent : c.info}>
          {isLeaderHighlighted ? <strong>team-lead</strong> : 'team-lead'}
        </text>
        {!isLeaderForegrounded && leaderVerb ? (
          <text fg={c.dim}>: {leaderVerb}\u2026</text>
        ) : null}
        {!isLeaderForegrounded && !leaderVerb && leaderIdleText ? (
          <text fg={c.dim}>: {leaderIdleText}</text>
        ) : null}
        {leaderTokenCount !== undefined && leaderTokenCount > 0 ? (
          <text fg={isLeaderHighlighted ? c.text : c.dim}>
            {' '}
            \u00b7 {leaderTokenCount} tokens
          </text>
        ) : null}
        {isLeaderHighlighted ? (
          <text fg={c.dim}> \u00b7 {TEAMMATE_SELECT_HINT}</text>
        ) : null}
        {isLeaderSelected && !isLeaderForegrounded ? (
          <text fg={c.dim}> \u00b7 enter to view</text>
        ) : null}
      </box>

      {teammates.map((teammate, index) => (
        <TeammateSpinnerLine
          key={teammate.id}
          {...teammate}
          isLast={!isInSelectionMode && index === teammates.length - 1}
          isSelected={isInSelectionMode && selectedIndex === index}
          isForegrounded={foregroundedId === teammate.id}
          allIdle={allIdle}
        />
      ))}

      {isInSelectionMode ? <HideRow isSelected={isHideSelected} /> : null}
    </box>
  )
}

function HideRow({ isSelected }: { isSelected: boolean }): React.ReactElement {
  return (
    <box flexDirection="row" paddingLeft={3}>
      <text fg={isSelected ? c.accent : undefined}>
        {isSelected ? <strong>{POINTER}</strong> : ' '}
      </text>
      <text fg={isSelected ? c.text : c.dim}>
        {isSelected ? <strong>{'\u2558\u2550 '}</strong> : '\u2514\u2500 '}
      </text>
      <text fg={isSelected ? c.text : c.dim}>
        {isSelected ? <strong>hide</strong> : 'hide'}
      </text>
      {isSelected ? (
        <text fg={c.dim}> \u00b7 enter to collapse</text>
      ) : null}
    </box>
  )
}

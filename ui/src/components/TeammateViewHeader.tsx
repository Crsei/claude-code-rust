import React from 'react'
import { c } from '../theme.js'
import { useAppState } from '../store/app-store.js'

/**
 * Header shown when viewing a teammate's transcript. Ported from the
 * upstream
 * `ui/examples/upstream-patterns/src/components/TeammateViewHeader.tsx`.
 *
 * The upstream pulls a full `InProcessTeammateTask` object out of
 * AppState via `getViewedTeammateTask(s)`. The Rust port doesn't model
 * that task type yet, so we read two optional fields from the store
 * (`viewedTeammate.name`, `viewedTeammate.prompt`, `viewedTeammate.color`)
 * and fall back gracefully when they're absent.
 */

type ViewedTeammate = {
  name: string
  prompt?: string
  color?: string
}

export function TeammateViewHeader(): React.ReactElement | null {
  const viewedTeammate = useAppState(s => {
    const value = (s as unknown as { viewedTeammate?: ViewedTeammate })
      .viewedTeammate
    return value ?? null
  })

  if (!viewedTeammate) {
    return null
  }

  const nameColor = viewedTeammate.color ?? c.accent

  return (
    <box flexDirection="column" marginBottom={1}>
      <box flexDirection="row">
        <text>Viewing </text>
        <text>
          <strong>
            <span fg={nameColor}>@{viewedTeammate.name}</span>
          </strong>
        </text>
        <text fg={c.dim}>{' \u00b7 esc to return'}</text>
      </box>
      {viewedTeammate.prompt ? (
        <text fg={c.dim}>{viewedTeammate.prompt}</text>
      ) : null}
    </box>
  )
}

import React, { useEffect, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/WorktreeExitDialog.tsx`.
 *
 * Shown when the user exits a session started inside a git worktree.
 * Upstream reaches into `git status`, `cleanupWorktree`, and
 * `killTmuxSession` directly; the Rust port surfaces those capabilities
 * through injected callbacks so the component stays decoupled from the
 * concrete filesystem/tmux helpers (which live on the Rust daemon).
 *
 * Status machine mirrors upstream:
 *
 *   loading \u2192 asking \u2192 keeping|removing \u2192 done
 *
 * When `loadWorktreeSnapshot()` reports no changes AND no extra commits,
 * the dialog silently cleans up and transitions straight to `done`,
 * matching the upstream short-circuit path.
 */

export type WorktreeSessionInfo = {
  worktreePath: string
  worktreeBranch: string
  originalCwd: string
  tmuxSessionName?: string
}

export type WorktreeSnapshot = {
  changedFiles: string[]
  commitCount: number
}

type ResultMessage = string | undefined

type Props = {
  session: WorktreeSessionInfo | null
  /** Populate `{ changedFiles, commitCount }` for the current worktree. */
  loadSnapshot: (session: WorktreeSessionInfo) => Promise<WorktreeSnapshot>
  /** Commit worktree to "keep" state (saves session tracking, no cleanup). */
  keepWorktree: (session: WorktreeSessionInfo) => Promise<void>
  /** Remove the worktree directory and delete the branch. */
  cleanupWorktree: (session: WorktreeSessionInfo) => Promise<void>
  /** Tear down the tmux session associated with the worktree. */
  killTmuxSession?: (name: string) => Promise<void>
  /** Called when the flow finishes (either path). `result` is a human message. */
  onDone: (result?: ResultMessage) => void
  /** Called when the user hits Escape with `onCancel` wired up. */
  onCancel?: () => void
}

type Status = 'loading' | 'asking' | 'keeping' | 'removing' | 'done'

type SelectOption = {
  label: string
  value: string
  description?: string
}

function buildSubtitle(
  session: WorktreeSessionInfo,
  snap: WorktreeSnapshot,
): string {
  const { changedFiles, commitCount } = snap
  const branchName = session.worktreeBranch
  if (changedFiles.length > 0 && commitCount > 0) {
    return `You have ${changedFiles.length} uncommitted ${changedFiles.length === 1 ? 'file' : 'files'} and ${commitCount} ${commitCount === 1 ? 'commit' : 'commits'} on ${branchName}. All will be lost if you remove.`
  }
  if (changedFiles.length > 0) {
    return `You have ${changedFiles.length} uncommitted ${changedFiles.length === 1 ? 'file' : 'files'}. These will be lost if you remove the worktree.`
  }
  if (commitCount > 0) {
    return `You have ${commitCount} ${commitCount === 1 ? 'commit' : 'commits'} on ${branchName}. The branch will be deleted if you remove the worktree.`
  }
  return 'You are working in a worktree. Keep it to continue working there, or remove it to clean up.'
}

function buildOptions(
  session: WorktreeSessionInfo,
  snap: WorktreeSnapshot,
): { options: SelectOption[]; defaultValue: string } {
  const removeDescription =
    snap.changedFiles.length > 0 || snap.commitCount > 0
      ? 'All changes and commits will be lost.'
      : 'Clean up the worktree directory.'
  if (session.tmuxSessionName) {
    return {
      options: [
        {
          label: 'Keep worktree and tmux session',
          value: 'keep-with-tmux',
          description: `Stays at ${session.worktreePath}. Reattach with: tmux attach -t ${session.tmuxSessionName}`,
        },
        {
          label: 'Keep worktree, kill tmux session',
          value: 'keep-kill-tmux',
          description: `Keeps worktree at ${session.worktreePath}, terminates tmux session.`,
        },
        {
          label: 'Remove worktree and tmux session',
          value: 'remove-with-tmux',
          description: removeDescription,
        },
      ],
      defaultValue: 'keep-with-tmux',
    }
  }
  return {
    options: [
      {
        label: 'Keep worktree',
        value: 'keep',
        description: `Stays at ${session.worktreePath}`,
      },
      {
        label: 'Remove worktree',
        value: 'remove',
        description: removeDescription,
      },
    ],
    defaultValue: 'keep',
  }
}

export function WorktreeExitDialog({
  session,
  loadSnapshot,
  keepWorktree,
  cleanupWorktree,
  killTmuxSession,
  onDone,
  onCancel,
}: Props): React.ReactElement | null {
  const [status, setStatus] = useState<Status>('loading')
  const [snapshot, setSnapshot] = useState<WorktreeSnapshot | null>(null)
  const [resultMessage, setResultMessage] = useState<ResultMessage>(undefined)
  const [focusIndex, setFocusIndex] = useState(0)

  useEffect(() => {
    if (!session) {
      onDone('No active worktree session found')
      return
    }
    let cancelled = false
    ;(async () => {
      const snap = await loadSnapshot(session)
      if (cancelled) return
      setSnapshot(snap)
      if (snap.changedFiles.length === 0 && snap.commitCount === 0) {
        setStatus('removing')
        try {
          await cleanupWorktree(session)
          if (cancelled) return
          setResultMessage('Worktree removed (no changes)')
        } catch {
          if (cancelled) return
          setResultMessage('Worktree cleanup failed, exiting anyway')
        }
        setStatus('done')
        return
      }
      setStatus('asking')
    })()
    return () => {
      cancelled = true
    }
  }, [cleanupWorktree, loadSnapshot, onDone, session])

  useEffect(() => {
    if (status === 'done') {
      onDone(resultMessage)
    }
  }, [status, onDone, resultMessage])

  const selectConfig = session && snapshot ? buildOptions(session, snapshot) : null

  const handleSelect = async (value: string) => {
    if (!session || !snapshot) return
    const hasTmux = Boolean(session.tmuxSessionName)

    if (value === 'keep' || value === 'keep-with-tmux') {
      setStatus('keeping')
      await keepWorktree(session)
      if (hasTmux) {
        setResultMessage(
          `Worktree kept. Your work is saved at ${session.worktreePath} on branch ${session.worktreeBranch}. Reattach to tmux session with: tmux attach -t ${session.tmuxSessionName}`,
        )
      } else {
        setResultMessage(
          `Worktree kept. Your work is saved at ${session.worktreePath} on branch ${session.worktreeBranch}`,
        )
      }
      setStatus('done')
      return
    }

    if (value === 'keep-kill-tmux') {
      setStatus('keeping')
      if (session.tmuxSessionName) {
        await killTmuxSession?.(session.tmuxSessionName)
      }
      await keepWorktree(session)
      setResultMessage(
        `Worktree kept at ${session.worktreePath} on branch ${session.worktreeBranch}. Tmux session terminated.`,
      )
      setStatus('done')
      return
    }

    if (value === 'remove' || value === 'remove-with-tmux') {
      setStatus('removing')
      if (session.tmuxSessionName) {
        await killTmuxSession?.(session.tmuxSessionName)
      }
      try {
        await cleanupWorktree(session)
      } catch {
        setResultMessage('Worktree cleanup failed, exiting anyway')
        setStatus('done')
        return
      }
      const tmuxNote = hasTmux ? ' Tmux session terminated.' : ''
      const { commitCount, changedFiles } = snapshot
      if (commitCount > 0 && changedFiles.length > 0) {
        setResultMessage(
          `Worktree removed. ${commitCount} ${commitCount === 1 ? 'commit' : 'commits'} and uncommitted changes were discarded.${tmuxNote}`,
        )
      } else if (commitCount > 0) {
        setResultMessage(
          `Worktree removed. ${commitCount} ${commitCount === 1 ? 'commit' : 'commits'} on ${session.worktreeBranch} ${commitCount === 1 ? 'was' : 'were'} discarded.${tmuxNote}`,
        )
      } else if (changedFiles.length > 0) {
        setResultMessage(
          `Worktree removed. Uncommitted changes were discarded.${tmuxNote}`,
        )
      } else {
        setResultMessage(`Worktree removed.${tmuxNote}`)
      }
      setStatus('done')
    }
  }

  const handleCancel = () => {
    if (onCancel) {
      onCancel()
      return
    }
    void handleSelect('keep')
  }

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (status !== 'asking' || !selectConfig) return
    const name = event.name
    if (name === 'escape') {
      handleCancel()
      return
    }
    if (name === 'up') {
      setFocusIndex(i => (i - 1 + selectConfig.options.length) % selectConfig.options.length)
      return
    }
    if (name === 'down' || name === 'tab') {
      setFocusIndex(i => (i + 1) % selectConfig.options.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      const opt = selectConfig.options[focusIndex]
      if (opt) void handleSelect(opt.value)
    }
  })

  if (!session) return null
  if (status === 'loading' || status === 'done') return null

  if (status === 'keeping') {
    return (
      <box flexDirection="row" marginY={1}>
        <Spinner label="Keeping worktree\u2026" />
      </box>
    )
  }

  if (status === 'removing') {
    return (
      <box flexDirection="row" marginY={1}>
        <Spinner label="Removing worktree\u2026" />
      </box>
    )
  }

  if (!snapshot || !selectConfig) return null

  const subtitle = buildSubtitle(session, snapshot)

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      paddingX={2}
      paddingY={1}
      title="Exiting worktree session"
      titleAlignment="center"
    >
      <text fg={c.dim}>{subtitle}</text>
      <box marginTop={1} flexDirection="column">
        {selectConfig.options.map((opt, i) => {
          const isFocused = i === focusIndex
          return (
            <box key={opt.value} flexDirection="column">
              <text
                fg={isFocused ? c.bg : c.text}
                bg={isFocused ? c.textBright : undefined}
              >
                {isFocused ? '\u25B8 ' : '  '}
                {opt.label}
              </text>
              {opt.description ? (
                <text fg={c.dim}>    {opt.description}</text>
              ) : null}
            </box>
          )
        })}
      </box>
    </box>
  )
}

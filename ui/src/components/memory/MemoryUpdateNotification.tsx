import React from 'react'
import { homedir } from 'node:os'
import { relative } from 'node:path'
import { useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/memory/MemoryUpdateNotification.tsx`.
 *
 * Upstream reads `getCwd()` from its bootstrap state; cc-rust exposes
 * the working directory through `state.cwd` on the app store, so the
 * component subscribes there directly. `getRelativeMemoryPath` is
 * re-exported so callers that already use it from the sample tree can
 * swap the import without touching usage.
 */

export function getRelativeMemoryPath(path: string, cwd: string): string {
  const home = homedir()
  const relativeToHome = path.startsWith(home) ? '~' + path.slice(home.length) : null
  const relativeToCwd = path.startsWith(cwd) ? './' + relative(cwd, path) : null

  if (relativeToHome && relativeToCwd) {
    return relativeToHome.length <= relativeToCwd.length
      ? relativeToHome
      : relativeToCwd
  }
  return relativeToHome ?? relativeToCwd ?? path
}

type Props = {
  memoryPath: string
  /** Override the store's cwd — mostly useful in tests. */
  cwdOverride?: string
}

export function MemoryUpdateNotification({ memoryPath, cwdOverride }: Props) {
  const cwd = useAppState().cwd
  const displayPath = getRelativeMemoryPath(memoryPath, cwdOverride ?? cwd)

  return (
    <box flexDirection="column">
      <text fg={c.text} selectable>
        Memory updated in {displayPath} · /memory to edit
      </text>
    </box>
  )
}

import React from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/PackageManagerAutoUpdater.tsx`.
 *
 * Upstream polls GCS every 30 min and shows a "Run `brew upgrade …`"
 * nudge when a newer version is available. The Lite frontend has no
 * auto-update plumbing (the Rust side handles distribution), so the
 * component is purely declarative — the parent passes the detected
 * package manager + new version and the nudge renders. When no update
 * is pending the component renders `null`, matching upstream.
 */

export type PackageManager =
  | 'homebrew'
  | 'winget'
  | 'apk'
  | 'unknown'
  | 'npm'
  | 'pnpm'
  | 'yarn'
  | 'bun'

type Props = {
  /** When unset / false no nudge renders. */
  updateAvailable?: boolean
  packageManager?: PackageManager
  /** Currently-installed version (shown when `verbose`). */
  currentVersion?: string
  latestVersion?: string
  verbose?: boolean
}

function updateCommand(pm: PackageManager | undefined): string {
  switch (pm) {
    case 'homebrew':
      return 'brew upgrade claude-code'
    case 'winget':
      return 'winget upgrade Anthropic.ClaudeCode'
    case 'apk':
      return 'apk upgrade claude-code'
    case 'npm':
      return 'npm install -g @anthropic-ai/claude-code'
    case 'pnpm':
      return 'pnpm up -g @anthropic-ai/claude-code'
    case 'yarn':
      return 'yarn global upgrade @anthropic-ai/claude-code'
    case 'bun':
      return 'bun update -g @anthropic-ai/claude-code'
    default:
      return 'your package manager update command'
  }
}

export function PackageManagerAutoUpdater({
  updateAvailable,
  packageManager,
  currentVersion,
  latestVersion,
  verbose = false,
}: Props) {
  if (!updateAvailable) {
    return null
  }

  return (
    <box flexDirection="row" gap={1}>
      {verbose && currentVersion && (
        <text fg={c.dim}>
          currentVersion: {currentVersion}
          {latestVersion ? ` \u2192 ${latestVersion}` : ''}
        </text>
      )}
      <text fg={c.warning}>
        Update available! Run:{' '}
        <strong>{updateCommand(packageManager)}</strong>
      </text>
    </box>
  )
}

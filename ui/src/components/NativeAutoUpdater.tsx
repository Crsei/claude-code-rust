import React from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/NativeAutoUpdater.tsx`.
 *
 * The upstream component owns the full check→install lifecycle — it
 * polls GCS, downloads the native installer, and reports progress. In
 * Lite that work lives on the Rust side; the frontend only has to
 * render whatever status the backend reports via
 * `AutoUpdaterResult`. Parent passes the current snapshot; this
 * component decides what (if anything) to show.
 */

export type AutoUpdaterResult = {
  version: string | null
  status: 'success' | 'install_failed' | 'checking' | 'up_to_date'
}

type Props = {
  isUpdating: boolean
  autoUpdaterResult: AutoUpdaterResult | null
  /** Optional channel + current/latest version info surfaced only when
   *  `verbose` is true, matching upstream's dev-mode display. */
  verbose?: boolean
  currentVersion?: string
  latestVersion?: string
  channel?: string
  /** When true and the result is success, render the install banner. */
  showSuccessMessage?: boolean
  /** Optional max-version warning (shown to Anthropic-internal users
   *  upstream). Passed through as-is here. */
  maxVersionIssue?: string | null
  showMaxVersionIssue?: boolean
}

export function NativeAutoUpdater({
  isUpdating,
  autoUpdaterResult,
  verbose = false,
  currentVersion,
  latestVersion,
  channel = 'latest',
  showSuccessMessage = true,
  maxVersionIssue = null,
  showMaxVersionIssue = false,
}: Props) {
  const hasResult = !!autoUpdaterResult?.version
  const hasVersionInfo = !!currentVersion && !!latestVersion
  const shouldRender =
    !!maxVersionIssue || hasResult || (isUpdating && hasVersionInfo)

  if (!shouldRender) {
    return null
  }

  return (
    <box flexDirection="row" gap={1}>
      {verbose && hasVersionInfo && (
        <text fg={c.dim}>
          current: {currentVersion} · {channel}: {latestVersion}
        </text>
      )}

      {isUpdating && <text fg={c.dim}>Checking for updates</text>}

      {!isUpdating &&
        autoUpdaterResult?.status === 'success' &&
        showSuccessMessage &&
        autoUpdaterResult.version && (
          <text fg={c.success}>
            \u2713 Update installed · Restart to update
          </text>
        )}

      {autoUpdaterResult?.status === 'install_failed' && (
        <text fg={c.error}>
          \u2717 Auto-update failed · Try <strong>/status</strong>
        </text>
      )}

      {maxVersionIssue && showMaxVersionIssue && (
        <text fg={c.warning}>
          \u26A0 Known issue: {maxVersionIssue} · Run{' '}
          <strong>claude rollback --safe</strong> to downgrade
        </text>
      )}
    </box>
  )
}

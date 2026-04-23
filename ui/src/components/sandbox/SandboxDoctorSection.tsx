import React from 'react'
import { c } from '../../theme.js'
import { useSandboxAdapter } from './sandbox-adapter.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/sandbox/SandboxDoctorSection.tsx`.
 *
 * Summary card surfaced inside the `/status` / `/doctor` output. Same
 * three short-circuits upstream uses:
 *   - Unsupported platform → render nothing.
 *   - Sandbox not enabled in settings → render nothing.
 *   - No dep errors / warnings → render nothing.
 */

export function SandboxDoctorSection() {
  const { settings } = useSandboxAdapter()
  if (!settings.supportedPlatform) return null
  if (!settings.enabledInSettings) return null

  const { errors, warnings } = settings.dependencyCheck
  if (errors.length === 0 && warnings.length === 0) return null

  const hasErrors = errors.length > 0
  const statusColor = hasErrors ? c.error : c.warning
  const statusText = hasErrors ? 'Missing dependencies' : 'Available (with warnings)'

  return (
    <box flexDirection="column">
      <text>
        <strong>Sandbox</strong>
      </text>
      <text>
        \u2514 Status: <span fg={statusColor}>{statusText}</span>
      </text>
      {errors.map((error, i) => (
        <text key={`err-${i}`} fg={c.error} selectable>
          \u2514 {error}
        </text>
      ))}
      {warnings.map((warn, i) => (
        <text key={`warn-${i}`} fg={c.warning} selectable>
          \u2514 {warn}
        </text>
      ))}
      {hasErrors && (
        <text fg={c.dim}>\u2514 Run /sandbox for install instructions</text>
      )}
    </box>
  )
}

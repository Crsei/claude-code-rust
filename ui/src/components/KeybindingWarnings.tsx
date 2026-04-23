import React from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `KeybindingWarnings`
 * (`ui/examples/upstream-patterns/src/components/KeybindingWarnings.tsx`).
 *
 * Upstream pulls the cached validation list from
 * `utils/keybindings/loadUserBindings.ts` on the Node side. In cc-rust
 * the keybinding config is loaded by the Rust backend and streamed to
 * the frontend through the IPC `ready` event (`keybindings` field). When
 * the backend later forwards parse / validation warnings via a
 * dedicated event we can hook them in here — until then this component
 * accepts an explicit `warnings` prop so callers (e.g. a future
 * settings view) can feed it directly.
 */

export type KeybindingWarning = {
  severity: 'error' | 'warning'
  message: string
  suggestion?: string
}

type Props = {
  warnings: KeybindingWarning[]
  path?: string
}

export function KeybindingWarnings({ warnings, path }: Props) {
  if (warnings.length === 0) return null

  const errors = warnings.filter(w => w.severity === 'error')
  const warns = warnings.filter(w => w.severity === 'warning')
  const headerColor = errors.length > 0 ? c.error : c.warning

  return (
    <box flexDirection="column" marginTop={1} marginBottom={1}>
      <text fg={headerColor}>
        <strong>Keybinding Configuration Issues</strong>
      </text>
      {path && (
        <text fg={c.dim}>Location: {path}</text>
      )}
      <box marginLeft={1} flexDirection="column" marginTop={1}>
        {errors.map((error, i) => (
          <box key={`err-${i}`} flexDirection="column">
            <text fg={c.dim}>
              <span>{'\u2514 '}</span>
              <span fg={c.error}>[Error]</span>
              <span>{' '}{error.message}</span>
            </text>
            {error.suggestion && (
              <box marginLeft={3}>
                <text fg={c.dim}>{'\u2192 '}{error.suggestion}</text>
              </box>
            )}
          </box>
        ))}
        {warns.map((warning, i) => (
          <box key={`warn-${i}`} flexDirection="column">
            <text fg={c.dim}>
              <span>{'\u2514 '}</span>
              <span fg={c.warning}>[Warning]</span>
              <span>{' '}{warning.message}</span>
            </text>
            {warning.suggestion && (
              <box marginLeft={3}>
                <text fg={c.dim}>{'\u2192 '}{warning.suggestion}</text>
              </box>
            )}
          </box>
        ))}
      </box>
    </box>
  )
}

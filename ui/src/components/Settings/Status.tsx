import React from 'react'
import { useAppState } from '../../store/app-store.js'
import { c } from '../../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/Settings/Status.tsx`.
 *
 * Upstream pulls data from `getSessionId`, `getCwd`, subscriber
 * helpers, and the `buildXxxProperties(...)` family. cc-rust
 * consolidates the same information out of the app store (`cwd`,
 * `model`, `sessionId`, `subsystems`) plus the properties list that
 * the caller supplies via `extraSections`. Diagnostics are optional —
 * pass any the backend surfaces (sandbox, auto-updater, memory) and
 * the component renders them at the bottom.
 */

export type PropertyValue = string | number | React.ReactNode | string[]

export interface Property {
  label?: string
  value: PropertyValue
}

export interface Diagnostic {
  message: string
  /** `error` renders with the error colour + ✗, `warning` with the
   *  warning colour + ⚠, `info` with a dim `·`. */
  severity?: 'error' | 'warning' | 'info'
}

type Props = {
  /** Caller-provided additional property sections — upstream's
   *  "IDE / MCP / Sandbox / Setting Sources" blocks come through here
   *  so this component stays presentational. */
  extraSections?: Property[][]
  diagnostics?: Diagnostic[]
  /** Loading indicator for the Diagnostics list. */
  diagnosticsLoading?: boolean
}

export function Status({ extraSections = [], diagnostics, diagnosticsLoading }: Props) {
  const state = useAppState()

  const primary: Property[] = [
    { label: 'Session ID', value: state.sessionId || '—' },
    { label: 'cwd', value: state.cwd || '—' },
    { label: 'Editor mode', value: state.editorMode },
    { label: 'Vim', value: state.vimEnabled ? `on (${state.vimMode})` : 'off' },
    { label: 'View mode', value: state.viewMode },
  ]
  const secondary: Property[] = [
    { label: 'Model', value: state.model || '—' },
    { label: 'LSP servers', value: String(state.subsystems.lsp.length) },
    { label: 'MCP servers', value: String(state.subsystems.mcp.length) },
    { label: 'Plugins', value: String(state.subsystems.plugins.length) },
    { label: 'Skills', value: String(state.subsystems.skills.length) },
    { label: 'IDE', value: state.ide.connected ? 'connected' : 'not connected' },
  ]

  const sections: Property[][] = [primary, secondary, ...extraSections]

  return (
    <box flexDirection="column" paddingY={1} gap={1}>
      {sections.map(
        (properties, i) =>
          properties.length > 0 && (
            <box key={i} flexDirection="column">
              {properties.map(({ label, value }, j) => (
                <box key={j} flexDirection="row" gap={1}>
                  {label && (
                    <text>
                      <strong>{label}:</strong>
                    </text>
                  )}
                  <PropertyView value={value} />
                </box>
              ))}
            </box>
          ),
      )}

      {(diagnosticsLoading || (diagnostics && diagnostics.length > 0)) && (
        <box flexDirection="column">
          <text>
            <strong>System Diagnostics</strong>
          </text>
          {diagnosticsLoading && (
            <text fg={c.dim}>
              <em>Running diagnostics…</em>
            </text>
          )}
          {!diagnosticsLoading && diagnostics && diagnostics.length === 0 && (
            <text fg={c.dim}>
              <em>All checks passed</em>
            </text>
          )}
          {!diagnosticsLoading &&
            diagnostics &&
            diagnostics.map((diag, i) => (
              <box key={i} flexDirection="row" gap={1} paddingX={1}>
                <text fg={colorForSeverity(diag.severity)}>
                  {glyphForSeverity(diag.severity)}
                </text>
                <text selectable>{diag.message}</text>
              </box>
            ))}
        </box>
      )}

      <text fg={c.dim}>
        <em>Esc to cancel</em>
      </text>
    </box>
  )
}

function PropertyView({ value }: { value: PropertyValue }) {
  if (Array.isArray(value)) {
    return (
      <box flexDirection="row" gap={1}>
        {value.map((item, i) => (
          <text key={i} selectable>
            {item}
            {i < value.length - 1 ? ',' : ''}
          </text>
        ))}
      </box>
    )
  }
  if (typeof value === 'string' || typeof value === 'number') {
    return <text selectable>{String(value)}</text>
  }
  return <>{value}</>
}

function colorForSeverity(sev?: Diagnostic['severity']): string {
  switch (sev) {
    case 'error':
      return c.error
    case 'warning':
      return c.warning
    default:
      return c.dim
  }
}

function glyphForSeverity(sev?: Diagnostic['severity']): string {
  switch (sev) {
    case 'error':
      return '\u2717' // ✗
    case 'warning':
      return '\u26A0' // ⚠
    default:
      return '\u00B7' // ·
  }
}

/**
 * Convenience hook — builds the default diagnostics list from whatever
 * lives in the store today. Callers can extend with their own backend
 * diagnostics by concatenating additional entries before passing into
 * `<Status diagnostics={...}/>`.
 */
export function buildDefaultDiagnostics(): Diagnostic[] {
  // Intentionally empty — cc-rust does not surface installation-health
  // or memory diagnostics over IPC yet. Host code that wires up the
  // /doctor backend should supply its own list.
  return []
}

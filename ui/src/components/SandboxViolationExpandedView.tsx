import React from 'react'
import { c } from '../theme.js'
import { useAppState } from '../store/app-store.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/SandboxViolationExpandedView.tsx`.
 *
 * Upstream subscribes to a `SandboxManager` event store that is not
 * wired into the Lite frontend. Violations are instead surfaced via the
 * shared `SubsystemState.sandbox_violations` snapshot the backend
 * pushes over IPC. If the app state has no violations (or the sandbox
 * subsystem isn't present), the view renders `null` — matching the
 * upstream "invisible until something is blocked" contract.
 */

export type SandboxViolationEvent = {
  timestamp: number
  command?: string
  line: string
}

const MAX_ROWS = 10

function formatTime(timestamp: number): string {
  const d = new Date(timestamp)
  const h = d.getHours() % 12 || 12
  const m = String(d.getMinutes()).padStart(2, '0')
  const s = String(d.getSeconds()).padStart(2, '0')
  const ampm = d.getHours() < 12 ? 'am' : 'pm'
  return `${h}:${m}:${s}${ampm}`
}

function readViolations(
  subsystems: Record<string, unknown> | undefined,
): { violations: SandboxViolationEvent[]; total: number } | null {
  const sandbox = subsystems?.sandbox as
    | { violations?: SandboxViolationEvent[]; total?: number }
    | undefined
  if (!sandbox) return null
  const violations = Array.isArray(sandbox.violations) ? sandbox.violations : []
  const total = typeof sandbox.total === 'number' ? sandbox.total : violations.length
  return { violations, total }
}

export function SandboxViolationExpandedView() {
  const state = useAppState() as unknown as { subsystems?: Record<string, unknown> }
  const snapshot = readViolations(state.subsystems)

  if (!snapshot || snapshot.total === 0) {
    return null
  }

  const { violations, total } = snapshot
  const recent = violations.slice(-MAX_ROWS)
  const suffix = total === 1 ? 'operation' : 'operations'

  return (
    <box flexDirection="column" marginTop={1}>
      <box>
        <text fg={c.warning}>
          {`\u29C8 Sandbox blocked ${total} total ${suffix}`}
        </text>
      </box>
      {recent.map((v, i) => (
        <box key={`${v.timestamp}-${i}`} paddingLeft={2}>
          <text fg={c.dim}>
            {formatTime(v.timestamp)}
            {v.command ? ` ${v.command}:` : ''} {v.line}
          </text>
        </box>
      ))}
      <box paddingLeft={2}>
        <text fg={c.dim}>
          … showing last {Math.min(MAX_ROWS, recent.length)} of {total}
        </text>
      </box>
    </box>
  )
}

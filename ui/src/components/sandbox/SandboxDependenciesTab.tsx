import React from 'react'
import { c } from '../../theme.js'
import type { SandboxDependencyCheck } from './sandbox-adapter.js'
import { useSandboxAdapter } from './sandbox-adapter.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/sandbox/SandboxDependenciesTab.tsx`.
 *
 * Same breakdown upstream ships:
 * - macOS: `seatbelt` (built-in) + ripgrep.
 * - Linux / WSL: ripgrep + bwrap + socat + optional seccomp.
 * - Anywhere else: whatever the backend reports as an error is printed
 *   verbatim so new platform errors aren't silently swallowed.
 *
 * The platform string comes from the adapter rather than from the
 * frontend's `process.platform` — the Lite frontend runs in the same
 * process as Rust, but the sandbox semantics are decided there.
 */

type Props = {
  /** Allow callers to override the adapter's check payload — used by
   *  the Settings shell to pass a cached/fresh diag result without
   *  re-running the full sandbox discovery. */
  depCheck?: SandboxDependencyCheck
}

export function SandboxDependenciesTab({ depCheck }: Props = {}) {
  const { settings } = useSandboxAdapter()
  const check = depCheck ?? settings.dependencyCheck
  const isMac = settings.platform === 'macos'

  const rgMissing = check.errors.some(e => e.includes('ripgrep'))
  const bwrapMissing = check.errors.some(e => e.includes('bwrap'))
  const socatMissing = check.errors.some(e => e.includes('socat'))
  const seccompMissing = check.warnings.length > 0

  const otherErrors = check.errors.filter(
    e => !e.includes('ripgrep') && !e.includes('bwrap') && !e.includes('socat'),
  )

  const rgInstallHint = isMac ? 'brew install ripgrep' : 'apt install ripgrep'

  return (
    <box flexDirection="column" paddingY={1} gap={1}>
      {isMac && (
        <box flexDirection="column">
          <text>
            seatbelt: <span fg={c.success}>built-in (macOS)</span>
          </text>
        </box>
      )}

      <box flexDirection="column">
        <text>
          ripgrep (rg):{' '}
          {rgMissing ? (
            <span fg={c.error}>not found</span>
          ) : (
            <span fg={c.success}>found</span>
          )}
        </text>
        {rgMissing && (
          <text fg={c.dim}>
            {'  '}· {rgInstallHint}
          </text>
        )}
      </box>

      {!isMac && (
        <>
          <box flexDirection="column">
            <text>
              bubblewrap (bwrap):{' '}
              {bwrapMissing ? (
                <span fg={c.error}>not installed</span>
              ) : (
                <span fg={c.success}>installed</span>
              )}
            </text>
            {bwrapMissing && (
              <text fg={c.dim}>{'  '}· apt install bubblewrap</text>
            )}
          </box>

          <box flexDirection="column">
            <text>
              socat:{' '}
              {socatMissing ? (
                <span fg={c.error}>not installed</span>
              ) : (
                <span fg={c.success}>installed</span>
              )}
            </text>
            {socatMissing && <text fg={c.dim}>{'  '}· apt install socat</text>}
          </box>

          <box flexDirection="column">
            <text>
              seccomp filter:{' '}
              {seccompMissing ? (
                <span fg={c.warning}>not installed</span>
              ) : (
                <span fg={c.success}>installed</span>
              )}
              {seccompMissing && (
                <span fg={c.dim}> (required to block unix domain sockets)</span>
              )}
            </text>
            {seccompMissing && (
              <box flexDirection="column">
                <text fg={c.dim}>
                  {'  '}· npm install -g @anthropic-ai/sandbox-runtime
                </text>
                <text fg={c.dim}>
                  {'  '}· or copy vendor/seccomp/* from sandbox-runtime and set
                </text>
                <text fg={c.dim}>
                  {'    '}sandbox.seccomp.bpfPath and applyPath in settings.json
                </text>
              </box>
            )}
          </box>
        </>
      )}

      {otherErrors.map(err => (
        <text key={err} fg={c.error} selectable>
          {err}
        </text>
      ))}
    </box>
  )
}

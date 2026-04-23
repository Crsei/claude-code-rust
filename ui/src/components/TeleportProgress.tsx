import React, { useEffect, useState } from 'react'
import { c } from '../theme.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/TeleportProgress.tsx`.
 *
 * Progress UI shown while `teleportResumeCodeSession` is walking through
 * the validate \u2192 fetch logs \u2192 fetch branch \u2192 checkout sequence. The
 * upstream component also ships the imperative `teleportWithProgress()`
 * helper which renders itself into an Ink `Root`; that driver has no
 * equivalent in OpenTUI-React-land — the Rust frontend mounts this
 * component directly and updates `currentStep` from its store.
 *
 * Animation uses a React interval instead of Ink's `useAnimationFrame`
 * because the OpenTUI renderer drives paints from its own dirty list;
 * a 100ms tick is enough to match upstream's frame cadence.
 */

export type TeleportProgressStep =
  | 'validating'
  | 'fetching_logs'
  | 'fetching_branch'
  | 'checking_out'

const SPINNER_FRAMES = ['\u25D0', '\u25D3', '\u25D1', '\u25D2']
const TICK = '\u2713'
const CIRCLE = '\u25CB'
const SPINNER_INTERVAL_MS = 100

const STEPS: { key: TeleportProgressStep; label: string }[] = [
  { key: 'validating', label: 'Validating session' },
  { key: 'fetching_logs', label: 'Fetching session logs' },
  { key: 'fetching_branch', label: 'Getting branch info' },
  { key: 'checking_out', label: 'Checking out branch' },
]

type Props = {
  currentStep: TeleportProgressStep
  sessionId?: string
}

export function TeleportProgress({
  currentStep,
  sessionId,
}: Props): React.ReactElement {
  const [frame, setFrame] = useState(0)

  useEffect(() => {
    const id = setInterval(() => {
      setFrame(f => (f + 1) % SPINNER_FRAMES.length)
    }, SPINNER_INTERVAL_MS)
    return () => clearInterval(id)
  }, [])

  const currentStepIndex = STEPS.findIndex(s => s.key === currentStep)

  return (
    <box flexDirection="column" paddingX={1} paddingY={1}>
      <box marginBottom={1}>
        <text>
          <strong>
            <span fg={c.accent}>
              {SPINNER_FRAMES[frame]} Teleporting session\u2026
            </span>
          </strong>
        </text>
      </box>

      {sessionId && (
        <box marginBottom={1}>
          <text fg={c.dim}>{sessionId}</text>
        </box>
      )}

      <box flexDirection="column" marginLeft={2}>
        {STEPS.map((step, index) => {
          const isComplete = index < currentStepIndex
          const isCurrent = index === currentStepIndex
          const isPending = index > currentStepIndex

          let icon: string
          let color: string | undefined

          if (isComplete) {
            icon = TICK
            color = c.success
          } else if (isCurrent) {
            icon = SPINNER_FRAMES[frame]!
            color = c.accent
          } else {
            icon = CIRCLE
            color = c.dim
          }

          return (
            <box key={step.key} flexDirection="row">
              <box width={2}>
                <text fg={color}>{icon}</text>
              </box>
              <text fg={isPending ? c.dim : undefined}>
                {isCurrent ? <strong>{step.label}</strong> : step.label}
              </text>
            </box>
          )
        })}
      </box>
    </box>
  )
}

import React, {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from 'react'
import { c } from '../../../theme.js'
import type { AgentWizardData, WizardApi, WizardStepComponent } from './types.js'

/**
 * Minimal wizard framework for the agents create flow. Mirrors the upstream
 * `WizardProvider` contract (`useWizard()` + a flat step array) so each
 * step file can stay a one-for-one port of the reference.
 *
 * Intentional differences from upstream:
 * - No analytics event hooks — those were Ant-only.
 * - No `showStepCounter` rendering toggle — we always show `step N/M` in
 *   the footer since terminal real-estate is cheap.
 * - The chrome (dialog border + title) is provided by the caller
 *   (`AgentsDialog`), not the wizard. This keeps the focus/modal model
 *   consistent with the rest of the panel.
 */

interface Props {
  steps: WizardStepComponent[]
  initialData?: Partial<AgentWizardData>
  onComplete: () => void
  onCancel: () => void
  /** Shown above the current step; defaults to "Create new agent". */
  title?: string
}

interface InternalContext {
  api: WizardApi<AgentWizardData>
  stepIndex: number
  stepCount: number
  title: string
}

const Ctx = createContext<InternalContext | null>(null)

export function WizardProvider({
  steps,
  initialData,
  onComplete,
  onCancel,
  title = 'Create new agent',
}: Props) {
  const [data, setData] = useState<AgentWizardData>(() => ({ ...initialData }))
  const [index, setIndex] = useState(0)

  const updateWizardData = useCallback((patch: Partial<AgentWizardData>) => {
    setData(prev => ({ ...prev, ...patch }))
  }, [])

  const goNext = useCallback(() => {
    setIndex(current => {
      const next = current + 1
      if (next >= steps.length) {
        onComplete()
        return current
      }
      return next
    })
  }, [steps.length, onComplete])

  const goBack = useCallback(() => {
    setIndex(current => (current > 0 ? current - 1 : current))
  }, [])

  const goToStep = useCallback(
    (target: number) => {
      const clamped = Math.max(0, Math.min(steps.length - 1, target))
      setIndex(clamped)
    },
    [steps.length],
  )

  const cancel = useCallback(() => {
    onCancel()
  }, [onCancel])

  const api = useMemo<WizardApi<AgentWizardData>>(
    () => ({
      wizardData: data,
      updateWizardData,
      goNext,
      goBack,
      goToStep,
      cancel,
    }),
    [data, updateWizardData, goNext, goBack, goToStep, cancel],
  )

  const ctxValue = useMemo<InternalContext>(
    () => ({ api, stepIndex: index, stepCount: steps.length, title }),
    [api, index, steps.length, title],
  )
  const StepFn = steps[index]

  return (
    <Ctx.Provider value={ctxValue}>
      <box flexDirection="column" flexGrow={1}>
        <box flexDirection="row">
          <text>
            <strong><span fg={c.accent}>{title}</span></strong>
            <span fg={c.dim}>{'  ·  '}</span>
            <span fg={c.dim}>{`step ${index + 1} / ${steps.length}`}</span>
          </text>
        </box>
        <box marginTop={1} flexGrow={1}>
          {StepFn ? React.createElement(StepFn) : null}
        </box>
      </box>
    </Ctx.Provider>
  )
}

/** Hook used by each step to read and mutate the shared wizard state. */
export function useWizard(): WizardApi<AgentWizardData> {
  const ctx = useContext(Ctx)
  if (!ctx) {
    throw new Error('useWizard must be used inside <WizardProvider>')
  }
  return ctx.api
}

/** Optional subtitle + footer wrapper mirroring upstream `WizardDialogLayout`. */
export function WizardStepLayout({
  subtitle,
  children,
  footer,
}: {
  subtitle: ReactNode
  children: ReactNode
  footer?: ReactNode
}) {
  return (
    <box flexDirection="column" flexGrow={1}>
      <text>
        <span fg={c.info}>{subtitle}</span>
      </text>
      <box marginTop={1} flexDirection="column" flexGrow={1}>
        {children}
      </box>
      {footer ? (
        <box marginTop={1}>
          <text><span fg={c.dim}>{footer}</span></text>
        </box>
      ) : null}
    </box>
  )
}

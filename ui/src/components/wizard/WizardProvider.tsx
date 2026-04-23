import React, {
  createContext,
  type ReactNode,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from 'react'
import type { WizardContextValue, WizardProviderProps } from './types.js'

/**
 * Ported from
 * `ui/examples/upstream-patterns/src/components/wizard/WizardProvider.tsx`.
 *
 * Generic multi-step wizard container. Each step is a zero-props React
 * component that pulls shared state via `useWizard()`. Matches the
 * upstream navigation semantics byte-for-byte:
 *
 *  - `goNext` advances, `goBack` returns to the previous step. When
 *    non-linear transitions have pushed to `navigationHistory`, `goBack`
 *    pops from the stack rather than just decrementing.
 *  - Reaching past the last step sets `isCompleted`, which triggers
 *    `onComplete(wizardData)` in a `useEffect`. That effect-driven
 *    completion lets the parent mutate React state without colliding
 *    with the wizard's own render.
 *  - `onCancel` fires either from `cancel()` (explicit) or from
 *    `goBack()` at the first step (implicit).
 *
 * The caller owns the chrome — see `WizardDialogLayout` for the
 * companion component that renders the dialog frame + footer.
 */

export const WizardContext =
  createContext<WizardContextValue | null>(null)

export function WizardProvider<
  T extends Record<string, unknown> = Record<string, unknown>,
>({
  steps,
  initialData = {} as T,
  onComplete,
  onCancel,
  children,
  title,
  showStepCounter = true,
}: WizardProviderProps<T>): ReactNode {
  const [currentStepIndex, setCurrentStepIndex] = useState(0)
  const [wizardData, setWizardData] = useState<T>(initialData)
  const [isCompleted, setIsCompleted] = useState(false)
  const [navigationHistory, setNavigationHistory] = useState<number[]>([])

  useEffect(() => {
    if (isCompleted) {
      setNavigationHistory([])
      void onComplete(wizardData)
    }
  }, [isCompleted, wizardData, onComplete])

  const goNext = useCallback(() => {
    if (currentStepIndex < steps.length - 1) {
      if (navigationHistory.length > 0) {
        setNavigationHistory(prev => [...prev, currentStepIndex])
      }
      setCurrentStepIndex(prev => prev + 1)
    } else {
      setIsCompleted(true)
    }
  }, [currentStepIndex, steps.length, navigationHistory])

  const goBack = useCallback(() => {
    if (navigationHistory.length > 0) {
      const previousStep = navigationHistory[navigationHistory.length - 1]
      if (previousStep !== undefined) {
        setNavigationHistory(prev => prev.slice(0, -1))
        setCurrentStepIndex(previousStep)
      }
    } else if (currentStepIndex > 0) {
      setCurrentStepIndex(prev => prev - 1)
    } else {
      onCancel()
    }
  }, [currentStepIndex, navigationHistory, onCancel])

  const goToStep = useCallback(
    (index: number) => {
      if (index >= 0 && index < steps.length) {
        setNavigationHistory(prev => [...prev, currentStepIndex])
        setCurrentStepIndex(index)
      }
    },
    [currentStepIndex, steps.length],
  )

  const cancel = useCallback(() => {
    setNavigationHistory([])
    onCancel()
  }, [onCancel])

  const updateWizardData = useCallback((updates: Partial<T>) => {
    setWizardData(prev => ({ ...prev, ...updates }))
  }, [])

  const contextValue = useMemo<WizardContextValue<T>>(
    () => ({
      currentStepIndex,
      totalSteps: steps.length,
      wizardData,
      setWizardData,
      updateWizardData,
      goNext,
      goBack,
      goToStep,
      cancel,
      title,
      showStepCounter,
    }),
    [
      currentStepIndex,
      steps.length,
      wizardData,
      updateWizardData,
      goNext,
      goBack,
      goToStep,
      cancel,
      title,
      showStepCounter,
    ],
  )

  const CurrentStepComponent = steps[currentStepIndex]

  if (!CurrentStepComponent || isCompleted) {
    return null
  }

  return (
    <WizardContext.Provider value={contextValue as WizardContextValue}>
      {children ?? <CurrentStepComponent />}
    </WizardContext.Provider>
  )
}

import type { ReactNode } from 'react'

/**
 * Shared types for the generic wizard framework. Ported from
 * `ui/examples/upstream-patterns/src/components/wizard/types.ts` — upstream
 * ships that file as an auto-generated stub (`export type ... = any`), so
 * this file fills in the concrete shapes every call site actually needs.
 *
 * The `agent-settings/wizard/` bundle has a local copy of this contract
 * tailored to the agent-create flow; this one stays generic so any future
 * multi-step dialog (MCP install, theme onboarding, etc.) can reuse it.
 */

export type WizardStepComponent = React.ComponentType<unknown>

export interface WizardContextValue<
  T extends Record<string, unknown> = Record<string, unknown>,
> {
  /** 0-based index of the step currently on screen. */
  currentStepIndex: number
  /** Total step count. */
  totalSteps: number
  /** Full wizard state snapshot. */
  wizardData: T
  /** Replace the full state (use sparingly — prefer `updateWizardData`). */
  setWizardData: React.Dispatch<React.SetStateAction<T>>
  /** Merge a partial update into the shared state. */
  updateWizardData: (updates: Partial<T>) => void
  /** Advance to the next step, or fire `onComplete` if already on the last. */
  goNext: () => void
  /** Return to the previous step (or cancel if this was the first). */
  goBack: () => void
  /** Jump to an arbitrary step. */
  goToStep: (index: number) => void
  /** Cancel the wizard entirely. */
  cancel: () => void
  /** Title shown above the steps (the dialog layout renders this). */
  title?: string
  /** When false, the `(step N/M)` suffix is hidden. Defaults to true. */
  showStepCounter?: boolean
}

export interface WizardProviderProps<
  T extends Record<string, unknown> = Record<string, unknown>,
> {
  steps: WizardStepComponent[]
  initialData?: T
  onComplete: (data: T) => void
  onCancel: () => void
  children?: ReactNode
  title?: string
  showStepCounter?: boolean
}

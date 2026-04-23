import { useContext } from 'react'
import type { WizardContextValue } from './types.js'
import { WizardContext } from './WizardProvider.js'

/**
 * Hook that exposes the current wizard state + navigation actions.
 * Ported from
 * `ui/examples/upstream-patterns/src/components/wizard/useWizard.ts`.
 *
 * Throws when invoked outside a `<WizardProvider>` — upstream does the
 * same so misuses surface immediately during development.
 */
export function useWizard<
  T extends Record<string, unknown> = Record<string, unknown>,
>(): WizardContextValue<T> {
  const context = useContext(WizardContext) as WizardContextValue<T> | null
  if (!context) {
    throw new Error('useWizard must be used within a WizardProvider')
  }
  return context
}

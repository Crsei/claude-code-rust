/**
 * Barrel for the generic wizard framework. Mirrors the upstream layout in
 * `ui/examples/upstream-patterns/src/components/wizard/` — exports the
 * provider, hook, dialog layout, footer, and shared types.
 *
 * The `agent-settings/wizard/` folder carries a separate copy tailored
 * to the create-agent flow; both are intentional — the agent wizard
 * predates this generic one and they coexist so neither has to grow
 * into the other's use case.
 */
export type {
  WizardContextValue,
  WizardProviderProps,
  WizardStepComponent,
} from './types.js'
export { useWizard } from './useWizard.js'
export { WizardDialogLayout } from './WizardDialogLayout.js'
export { WizardNavigationFooter } from './WizardNavigationFooter.js'
export { WizardProvider } from './WizardProvider.js'

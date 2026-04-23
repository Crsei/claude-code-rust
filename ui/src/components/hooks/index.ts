/**
 * Barrel for the `/hooks` menu. Mirrors the upstream layout at
 * `ui/examples/upstream-patterns/src/components/hooks/`.
 */
export { HooksConfigMenu } from './HooksConfigMenu.js'
export type { HooksSnapshot } from './HooksConfigMenu.js'
export { PromptDialog } from './PromptDialog.js'
export { SelectEventMode } from './SelectEventMode.js'
export { SelectHookMode } from './SelectHookMode.js'
export { SelectMatcherMode } from './SelectMatcherMode.js'
export { ViewHookMode } from './ViewHookMode.js'
export type {
  HookConfigPayload,
  HookConfigType,
  HookEvent,
  HookEventMetadata,
  HookSource,
  IndividualHookConfig,
  PromptHookRequest,
} from './types.js'
export {
  getHookDisplayText,
  hookSourceDescriptionDisplayString,
  hookSourceHeaderDisplayString,
  hookSourceInlineDisplayString,
  plural,
} from './types.js'

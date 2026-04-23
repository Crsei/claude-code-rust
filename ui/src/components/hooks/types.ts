/**
 * Shared types for the `/hooks` read-only configuration menu.
 *
 * OpenTUI-native port of the upstream hook UI types that in the
 * upstream tree are split across
 * `src/utils/hooks/hooksConfigManager.ts`,
 * `src/utils/hooks/hooksSettings.ts`, `src/types/hooks.ts`,
 * and `src/entrypoints/agentSdkTypes.ts`.
 *
 * The Lite port consolidates just the shapes the UI needs so each
 * component stays self-contained and the Rust backend can produce a
 * flat snapshot without replicating the upstream type tree.
 */

export type HookEvent =
  | 'PreToolUse'
  | 'PostToolUse'
  | 'UserPromptSubmit'
  | 'Notification'
  | 'Stop'
  | 'SubagentStop'
  | 'PreCompact'
  | 'SessionStart'
  | 'SessionEnd'
  | 'StatusLine'

export type HookSource =
  | 'userSettings'
  | 'projectSettings'
  | 'localSettings'
  | 'policySettings'
  | 'pluginHook'

export type HookConfigType = 'command' | 'prompt' | 'agent' | 'http'

export type HookConfigPayload =
  | { type: 'command'; command: string; statusMessage?: string }
  | { type: 'prompt'; prompt: string; statusMessage?: string }
  | { type: 'agent'; prompt: string; statusMessage?: string }
  | { type: 'http'; url: string; statusMessage?: string }

export type IndividualHookConfig = {
  event: HookEvent
  matcher?: string
  source: HookSource
  pluginName?: string
  config: HookConfigPayload
}

export type HookEventMetadata = {
  description: string
  summary: string
  /** Populated only for events that accept tool-name matchers. */
  matcherMetadata?: {
    description: string
    placeholder?: string
  }
}

/**
 * Prompt-style hook request (for the `PromptDialog` view) \u2014 matches the
 * upstream `PromptRequest` shape without pulling in the Rust-side union.
 */
export type PromptHookRequest = {
  title?: string
  message: string
  options: Array<{
    key: string
    label: string
    description?: string
  }>
}

export function hookSourceInlineDisplayString(source: HookSource): string {
  switch (source) {
    case 'userSettings':
      return 'user'
    case 'projectSettings':
      return 'project'
    case 'localSettings':
      return 'local'
    case 'policySettings':
      return 'managed'
    case 'pluginHook':
      return 'plugin'
  }
}

export function hookSourceHeaderDisplayString(source: HookSource): string {
  switch (source) {
    case 'userSettings':
      return 'User settings'
    case 'projectSettings':
      return 'Project settings'
    case 'localSettings':
      return 'Local settings'
    case 'policySettings':
      return 'Managed settings'
    case 'pluginHook':
      return 'Plugin'
  }
}

export function hookSourceDescriptionDisplayString(source: HookSource): string {
  switch (source) {
    case 'userSettings':
      return 'from ~/.claude/settings.json'
    case 'projectSettings':
      return 'from .claude/settings.json'
    case 'localSettings':
      return 'from .claude/settings.local.json'
    case 'policySettings':
      return 'from managed policy settings'
    case 'pluginHook':
      return 'provided by a plugin'
  }
}

export function getHookDisplayText(config: HookConfigPayload): string {
  switch (config.type) {
    case 'command':
      return config.command
    case 'prompt':
    case 'agent':
      return config.prompt
    case 'http':
      return config.url
  }
}

export function plural(n: number, singular: string, pluralForm?: string): string {
  if (n === 1) return singular
  return pluralForm ?? `${singular}s`
}

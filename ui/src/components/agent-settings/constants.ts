import type { AgentDefinitionSource } from '../../ipc/protocol.js'

/**
 * Fixed color palette exposed by the editor. Matches the eight-color set in
 * the upstream upstream-patterns example; `""` means "unset / inherit".
 */
export const COLOR_CHOICES: ReadonlyArray<{ value: string; label: string }> = [
  { value: '', label: '(default)' },
  { value: 'red', label: 'red' },
  { value: 'orange', label: 'orange' },
  { value: 'yellow', label: 'yellow' },
  { value: 'green', label: 'green' },
  { value: 'blue', label: 'blue' },
  { value: 'purple', label: 'purple' },
  { value: 'pink', label: 'pink' },
  { value: 'cyan', label: 'cyan' },
]

/**
 * Model aliases the backend's `resolve_model_alias` understands plus the
 * common long-form IDs. Empty value = "inherit from parent session".
 */
export const MODEL_CHOICES: ReadonlyArray<{ value: string; label: string }> = [
  { value: '', label: '(inherit)' },
  { value: 'sonnet', label: 'sonnet' },
  { value: 'opus', label: 'opus' },
  { value: 'haiku', label: 'haiku' },
]

/** Short human label for a source — used in lists. */
export function sourceLabel(source: AgentDefinitionSource): string {
  switch (source.kind) {
    case 'builtin':
      return 'built-in'
    case 'user':
      return 'user'
    case 'project':
      return 'project'
    case 'plugin':
      return source.id ? `plugin:${source.id}` : 'plugin'
  }
}

/** Display color for the source tag. */
export function sourceColor(source: AgentDefinitionSource): string {
  switch (source.kind) {
    case 'builtin':
      return '#6C7086'
    case 'user':
      return '#89B4FA'
    case 'project':
      return '#A6E3A1'
    case 'plugin':
      return '#F9E2AF'
  }
}

/** Non-editable sources (builtin, plugin) must surface as read-only in the UI. */
export function isEditableSource(source: AgentDefinitionSource): boolean {
  return source.kind === 'user' || source.kind === 'project'
}

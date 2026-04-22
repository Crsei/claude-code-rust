/**
 * Adapter for `permission_request` protocol messages. Normalizes the
 * backend's plain-string option list into a structured form and assigns
 * a `PermissionCategory` so the migrated permission UI can branch on a
 * closed set instead of matching tool names itself.
 */

import type { PermissionRequest } from '../store/app-state.js'
import type {
  PermissionCategory,
  PermissionOption,
  PermissionRequestViewModel,
} from '../view-model/types.js'

export function mapPermissionRequestToViewModel(
  request: PermissionRequest,
): PermissionRequestViewModel {
  return {
    kind: 'permission_request',
    toolUseId: request.toolUseId,
    tool: request.tool,
    command: request.command,
    options: request.options.map(parsePermissionOption),
    category: categorizePermissionTool(request.tool),
  }
}

/**
 * Parse a single backend permission option string.
 *
 * The Rust permission system currently emits entries in either of these
 * forms:
 *   "Yes"
 *   "Yes (y)"
 *   "Always allow for this session (a)"
 *
 * We split off a trailing `(x)` hotkey when present and otherwise keep
 * the raw string as the label. The raw value is preserved as `value` so
 * the decision sent back over IPC stays byte-identical to the backend
 * option.
 */
export function parsePermissionOption(option: string): PermissionOption {
  const match = /^(.*?)\s*\(([a-z0-9])\)\s*$/i.exec(option)
  if (match) {
    return {
      value: option,
      label: match[1]!.trim(),
      hotkey: match[2]!.toLowerCase(),
    }
  }
  return { value: option, label: option.trim() }
}

export function categorizePermissionTool(tool: string): PermissionCategory {
  const name = tool.toLowerCase()
  if (name === 'bash' || name === 'powershell') {
    return 'bash'
  }
  if (
    name === 'edit' ||
    name === 'multiedit' ||
    name === 'fileedit' ||
    name === 'notebookedit'
  ) {
    return 'file_edit'
  }
  if (name === 'write' || name === 'filewrite') {
    return 'file_write'
  }
  if (name === 'webfetch' || name === 'web_fetch' || name === 'websearch') {
    return 'web_fetch'
  }
  return 'tool_generic'
}

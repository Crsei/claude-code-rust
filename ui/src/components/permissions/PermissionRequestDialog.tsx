import React, { useMemo, useState } from 'react'
import {
  extractFileEditContext,
  mapPermissionRequestToViewModel,
} from '../../adapters/index.js'
import { useBackend } from '../../ipc/context.js'
import { useAppDispatch } from '../../store/app-store.js'
import type { PermissionRequest } from '../../store/app-state.js'
import type { PermissionOption } from '../../view-model/types.js'
import { BashPermissionRequest } from './BashPermissionRequest.js'
import { FallbackPermissionRequest } from './FallbackPermissionRequest.js'
import { FileEditPermissionRequest } from './FileEditPermissionRequest.js'
import { FileWritePermissionRequest } from './FileWritePermissionRequest.js'
import { PermissionDialogFrame } from './PermissionDialogFrame.js'
import { PermissionPromptOptions } from './PermissionPromptOptions.js'
import { WebFetchPermissionRequest } from './WebFetchPermissionRequest.js'

/**
 * Category-aware permission dialog. Replaces the monolithic
 * `components/PermissionDialog.tsx` by:
 *
 * 1. Running the incoming `PermissionRequest` through the Issue 01
 *    adapter (`mapPermissionRequestToViewModel`) to get structured
 *    options + `PermissionCategory`.
 * 2. Picking a body variant under `./permissions/` based on the
 *    category (bash / file_edit / file_write / web_fetch / fallback).
 * 3. Delegating keyboard handling + button layout to
 *    `PermissionPromptOptions` so the chrome is shared across
 *    variants.
 *
 * Three backend fallbacks preserved from the old dialog:
 * - When `request.options` is empty the adapter's normalized option
 *   list will also be empty; we synthesize a sensible default triad
 *   (`Allow` / `Deny` / `Always Allow`).
 * - The decision we send over IPC stays the lower-snake-case form
 *   the Rust side already expects.
 * - Esc / the deny hotkey both reject.
 */

type Props = {
  request: PermissionRequest
}

const DEFAULT_OPTIONS: PermissionOption[] = [
  { value: 'Allow', label: 'Allow', hotkey: 'y' },
  { value: 'Deny', label: 'Deny', hotkey: 'n' },
  { value: 'Always Allow', label: 'Always Allow', hotkey: 'a' },
]

function toBackendDecision(option: PermissionOption): string {
  return option.value.toLowerCase().replace(/\s+/g, '_')
}

export function PermissionRequestDialog({ request }: Props) {
  const backend = useBackend()
  const dispatch = useAppDispatch()
  const [selected, setSelected] = useState(0)

  const viewModel = useMemo(
    () => mapPermissionRequestToViewModel(request),
    [request],
  )

  const options =
    viewModel.options.length > 0 ? viewModel.options : DEFAULT_OPTIONS

  const editContext = useMemo(() => {
    if (viewModel.category !== 'file_edit' && viewModel.category !== 'file_write') {
      return null
    }
    return extractFileEditContext(viewModel.tool, safeParseCommand(request.command))
  }, [viewModel.category, viewModel.tool, request.command])

  const decide = (option: PermissionOption) => {
    backend.send({
      type: 'permission_response',
      tool_use_id: request.toolUseId,
      decision: toBackendDecision(option),
    })
    dispatch({ type: 'PERMISSION_DISMISS' })
  }

  const cancel = () => {
    const denyOption =
      options.find(opt => /deny|reject|^no$/i.test(opt.label)) ?? options[0]!
    backend.send({
      type: 'permission_response',
      tool_use_id: request.toolUseId,
      decision: toBackendDecision(denyOption),
    })
    dispatch({ type: 'PERMISSION_DISMISS' })
  }

  return (
    <PermissionDialogFrame category={viewModel.category} tool={viewModel.tool}>
      {viewModel.category === 'bash' && (
        <BashPermissionRequest command={viewModel.command} />
      )}
      {viewModel.category === 'file_edit' && (
        <FileEditPermissionRequest
          context={editContext}
          fallbackCommand={viewModel.command}
        />
      )}
      {viewModel.category === 'file_write' && (
        <FileWritePermissionRequest
          context={editContext}
          fallbackCommand={viewModel.command}
        />
      )}
      {viewModel.category === 'web_fetch' && (
        <WebFetchPermissionRequest url={viewModel.command} />
      )}
      {viewModel.category === 'tool_generic' && (
        <FallbackPermissionRequest command={viewModel.command} />
      )}

      <box marginTop={1}>
        <PermissionPromptOptions
          options={options}
          selectedIndex={selected}
          onSelect={setSelected}
          onConfirm={decide}
          onCancel={cancel}
        />
      </box>
    </PermissionDialogFrame>
  )
}

/** The backend currently packs file-edit payloads into `command` as
 *  a JSON string. Return the parsed object when possible so the
 *  adapter can extract a `FileEditContext`. */
function safeParseCommand(command: string): unknown {
  if (!command) return undefined
  const trimmed = command.trim()
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) {
    return undefined
  }
  try {
    return JSON.parse(trimmed)
  } catch {
    return undefined
  }
}

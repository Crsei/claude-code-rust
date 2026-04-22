import React, { useEffect, useMemo, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { useBackend } from '../ipc/context.js'
import type {
  LspRecommendationDecision,
  LspRecommendationPayload,
} from '../ipc/protocol.js'
import { useAppDispatch } from '../store/app-store.js'
import { c } from '../theme.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/LspRecommendation/LspRecommendationMenu.tsx`.
 *
 * Shown when the backend emits an `LspEvent::recommendation_request`.
 * The user picks `yes` / `no` / `never` / `disable`; the choice is sent
 * back through `FrontendMessage::lsp_command` with kind
 * `recommendation_response`. A 30-second auto-dismiss timer counts as
 * `no` — matching the upstream behaviour.
 */

const AUTO_DISMISS_MS = 30_000

type Option = {
  value: LspRecommendationDecision
  label: string
  hotkey: string
}

type Props = {
  payload: LspRecommendationPayload
}

function buildOptions(pluginName: string): Option[] {
  return [
    { value: 'yes', label: `Yes, install ${pluginName}`, hotkey: 'y' },
    { value: 'no', label: 'No, not now', hotkey: 'n' },
    { value: 'never', label: `Never for ${pluginName}`, hotkey: 'x' },
    { value: 'disable', label: 'Disable all LSP recommendations', hotkey: 'd' },
  ]
}

export function LspRecommendationDialog({ payload }: Props) {
  const backend = useBackend()
  const dispatch = useAppDispatch()
  const [selected, setSelected] = useState(0)

  const options = useMemo(() => buildOptions(payload.plugin_name), [payload.plugin_name])
  const safeIndex = Math.max(0, Math.min(selected, options.length - 1))

  const respond = useRef<(decision: LspRecommendationDecision) => void>(() => {})
  respond.current = (decision: LspRecommendationDecision) => {
    backend.send({
      type: 'lsp_command',
      command: {
        kind: 'recommendation_response',
        request_id: payload.request_id,
        plugin_name: payload.plugin_name,
        decision,
      },
    })
    dispatch({ type: 'LSP_RECOMMENDATION_DISMISS' })
  }

  // 30s auto-dismiss — same as upstream. Counts as `no`.
  useEffect(() => {
    const timer = setTimeout(() => respond.current('no'), AUTO_DISMISS_MS)
    return () => clearTimeout(timer)
  }, [payload.request_id])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const input = (seq ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input) {
      const match = options.findIndex(opt => opt.hotkey === input)
      if (match >= 0) {
        respond.current(options[match]!.value)
        return
      }
    }

    if (name === 'escape') {
      respond.current('no')
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'down' || input === 'j') {
      setSelected(Math.min(options.length - 1, safeIndex + 1))
      return
    }
    if (name === 'tab') {
      setSelected((safeIndex + 1) % options.length)
      return
    }
    if (name === 'return' || name === 'enter') {
      const opt = options[safeIndex]
      if (opt) respond.current(opt.value)
    }
  })

  const triggerLabel = payload.language_id
    ? `${payload.file_extension} (${payload.language_id})`
    : payload.file_extension

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title="LSP Plugin Recommendation"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        <span fg={c.dim}>
          LSP provides code intelligence like go-to-definition and error checking.
        </span>
      </text>

      <box marginTop={1} flexDirection="column">
        <text>
          <span fg={c.dim}>Plugin: </span>
          <strong>{payload.plugin_name}</strong>
        </text>
        {payload.plugin_description && (
          <text>
            <span fg={c.dim}>{payload.plugin_description}</span>
          </text>
        )}
        <text>
          <span fg={c.dim}>Triggered by: </span>
          <span>{triggerLabel} files</span>
        </text>
      </box>

      <box marginTop={1}>
        <text>Would you like to install this LSP plugin?</text>
      </box>

      <box marginTop={1} flexDirection="column">
        {options.map((opt, i) => {
          const isSelected = i === safeIndex
          return (
            <box key={opt.value} flexDirection="row">
              <text fg={isSelected ? c.bg : undefined} bg={isSelected ? c.textBright : undefined}>
                <strong>{` ${opt.label} `}</strong>
              </text>
              <text fg={c.dim}> ({opt.hotkey})</text>
            </box>
          )
        })}
      </box>

      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>
              Up/Down to move · Enter to confirm · Hotkeys y/n/x/d · Esc = No · auto-dismiss in 30s
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}

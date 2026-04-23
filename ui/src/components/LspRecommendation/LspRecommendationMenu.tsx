import React, { useEffect, useMemo, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/LspRecommendation/LspRecommendationMenu.tsx`.
 *
 * Upstream uses this menu inside a `PermissionDialog` and drives
 * selection through the Ink `<Select>` primitive. cc-rust already
 * ships a wired dialog (`components/LspRecommendationDialog.tsx`) that
 * binds the same four decisions to the `LspEvent::recommendation_request`
 * IPC flow. This component is the presentational layer of that dialog
 * — the parent passes the plugin metadata + a callback, the menu draws
 * the four options and handles keyboard + the 30-second auto-dismiss
 * timer upstream also uses.
 *
 * The 30-second auto-dismiss mirrors upstream exactly — treating no
 * response as an implicit `'no'` keeps us aligned with the server's
 * recommendation-response contract.
 */

type Decision = 'yes' | 'no' | 'never' | 'disable'

type Props = {
  pluginName: string
  pluginDescription?: string
  fileExtension: string
  languageId?: string
  onResponse: (response: Decision) => void
  /** Override the dismiss timer — defaults to 30 000 ms. Pass `0` to
   *  disable. */
  autoDismissMs?: number
}

type Option = {
  value: Decision
  label: string
  hotkey: string
}

const AUTO_DISMISS_MS = 30_000

function buildOptions(pluginName: string): Option[] {
  return [
    { value: 'yes', label: `Yes, install ${pluginName}`, hotkey: 'y' },
    { value: 'no', label: 'No, not now', hotkey: 'n' },
    { value: 'never', label: `Never for ${pluginName}`, hotkey: 'x' },
    { value: 'disable', label: 'Disable all LSP recommendations', hotkey: 'd' },
  ]
}

export function LspRecommendationMenu({
  pluginName,
  pluginDescription,
  fileExtension,
  languageId,
  onResponse,
  autoDismissMs = AUTO_DISMISS_MS,
}: Props) {
  const [selected, setSelected] = useState(0)
  const options = useMemo(() => buildOptions(pluginName), [pluginName])
  const safeIndex = Math.max(0, Math.min(selected, options.length - 1))

  // `useRef` + write-on-render keeps the timeout stable across renders —
  // matches upstream's "latest ref" pattern.
  const onResponseRef = useRef(onResponse)
  onResponseRef.current = onResponse

  useEffect(() => {
    if (!Number.isFinite(autoDismissMs) || autoDismissMs <= 0) return undefined
    const timer = setTimeout(() => onResponseRef.current('no'), autoDismissMs)
    return () => clearTimeout(timer)
  }, [autoDismissMs])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input) {
      const match = options.findIndex(opt => opt.hotkey === input)
      if (match >= 0) {
        onResponseRef.current(options[match]!.value)
        return
      }
    }

    if (name === 'escape') {
      onResponseRef.current('no')
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
      onResponseRef.current(options[safeIndex]!.value)
    }
  })

  const triggerLabel = languageId
    ? `${fileExtension} (${languageId})`
    : fileExtension

  return (
    <box
      flexDirection="column"
      border
      borderStyle="rounded"
      borderColor={c.warning}
      title="LSP Plugin Recommendation"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text fg={c.dim}>
        LSP provides code intelligence like go-to-definition and error checking.
      </text>
      <box marginTop={1} flexDirection="column">
        <text>
          <span fg={c.dim}>Plugin: </span>
          <strong>{pluginName}</strong>
        </text>
        {pluginDescription && (
          <text fg={c.dim}>{pluginDescription}</text>
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
              Up/Down · Enter to confirm · y/n/x/d hotkeys · Esc = No ·
              auto-dismiss in {Math.round(autoDismissMs / 1000)}s
            </span>
          </em>
        </text>
      </box>
    </box>
  )
}

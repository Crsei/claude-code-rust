import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/IdeAutoConnectDialog.tsx`.
 *
 * Upstream drives two dialogs side-by-side: one that asks the user to
 * enable auto-connect, and a mirror that asks to disable it. Both save
 * their outcome directly to the global config file. cc-rust has no
 * direct config-writing hook from the frontend, so the port delegates
 * persistence: callers pass `onDone(enabled)` and drop the value into
 * whichever backend command owns the IDE preference.
 *
 * The two flavours are kept as named exports so the IDE onboarding flow
 * can pick the right phrasing without a runtime branch on each mount.
 * `shouldShowAutoConnectDialog` / `shouldShowDisableAutoConnectDialog`
 * are re-exported as pure predicates that consume an object describing
 * the current config + terminal support — again leaving the actual
 * config read to the caller.
 */

type Option = {
  value: 'yes' | 'no'
  label: string
  hotkey: string
}

const OPTIONS: Option[] = [
  { value: 'yes', label: 'Yes', hotkey: 'y' },
  { value: 'no', label: 'No', hotkey: 'n' },
]

type BaseProps = {
  onDone: (decision: 'yes' | 'no') => void
  /** Defaults to `'yes'` for the enable dialog and `'no'` for the
   *  disable dialog, matching upstream. */
  initialSelection?: 'yes' | 'no'
  /** Appended under the Select — upstream hard-codes the "/config or
   *  --ide flag" hint. Override when the host has different plumbing. */
  subtitle?: string
}

function DialogShell({
  title,
  subtitle,
  initialSelection = 'yes',
  onDone,
  onCancel,
}: BaseProps & { title: string; onCancel: () => void }) {
  const initialIndex = OPTIONS.findIndex(opt => opt.value === initialSelection)
  const [selected, setSelected] = useState(Math.max(0, initialIndex))
  const safeIndex = Math.max(0, Math.min(selected, OPTIONS.length - 1))

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const input = (event.sequence ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (input === 'y') {
      onDone('yes')
      return
    }
    if (input === 'n') {
      onDone('no')
      return
    }
    if (name === 'escape') {
      onCancel()
      return
    }
    if (name === 'left' || name === 'up' || input === 'h' || input === 'k') {
      setSelected(Math.max(0, safeIndex - 1))
      return
    }
    if (name === 'right' || name === 'down' || input === 'l' || input === 'j') {
      setSelected(Math.min(OPTIONS.length - 1, safeIndex + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      onDone(OPTIONS[safeIndex]!.value)
    }
  })

  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.info}
      title={title}
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="row" gap={2}>
        {OPTIONS.map((opt, i) => {
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
      {subtitle && (
        <box marginTop={1}>
          <text fg={c.dim} selectable>
            {subtitle}
          </text>
        </box>
      )}
      <box marginTop={1}>
        <text>
          <em>
            <span fg={c.dim}>←/→ · Enter to confirm · Esc to skip</span>
          </em>
        </text>
      </box>
    </box>
  )
}

type IdeAutoConnectProps = {
  onComplete: (autoConnect: boolean) => void
}

export function IdeAutoConnectDialog({ onComplete }: IdeAutoConnectProps) {
  const handle = (decision: 'yes' | 'no') => onComplete(decision === 'yes')
  return (
    <DialogShell
      title="Do you wish to enable auto-connect to IDE?"
      subtitle="You can also configure this in /config or with the --ide flag"
      initialSelection="yes"
      onDone={handle}
      onCancel={() => onComplete(false)}
    />
  )
}

type IdeDisableAutoConnectProps = {
  onComplete: (disableAutoConnect: boolean) => void
}

export function IdeDisableAutoConnectDialog({ onComplete }: IdeDisableAutoConnectProps) {
  const handle = (decision: 'yes' | 'no') => onComplete(decision === 'yes')
  return (
    <DialogShell
      title="Do you wish to disable auto-connect to IDE?"
      subtitle="You can also configure this in /config"
      initialSelection="no"
      onDone={handle}
      onCancel={() => onComplete(false)}
    />
  )
}

/**
 * Pure predicate matching upstream `shouldShowAutoConnectDialog`. Kept
 * as a standalone helper so the host decides where the config comes
 * from (Lite pipes it in as part of the onboarding state payload).
 */
export function shouldShowAutoConnectDialog(ctx: {
  isSupportedTerminal: boolean
  autoConnectIde: boolean | null | undefined
  hasShownDialog: boolean
}): boolean {
  return (
    !ctx.isSupportedTerminal &&
    ctx.autoConnectIde !== true &&
    !ctx.hasShownDialog
  )
}

export function shouldShowDisableAutoConnectDialog(ctx: {
  isSupportedTerminal: boolean
  autoConnectIde: boolean | null | undefined
}): boolean {
  return !ctx.isSupportedTerminal && ctx.autoConnectIde === true
}

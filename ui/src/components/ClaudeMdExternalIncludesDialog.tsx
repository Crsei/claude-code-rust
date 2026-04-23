import React, { useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * "Allow external CLAUDE.md file imports?" consent dialog.
 *
 * OpenTUI-native port of the upstream `ClaudeMdExternalIncludesDialog`
 * (`ui/examples/upstream-patterns/src/components/ClaudeMdExternalIncludesDialog.tsx`).
 * The upstream version mutates project config and logs analytics; the
 * Lite port surfaces the same prompt and reports `approved: boolean`
 * via `onDone`. Persisting the decision is the caller's responsibility.
 */

export type ExternalClaudeMdInclude = {
  path: string
}

type Props = {
  onDone: (approved: boolean) => void
  /** Hide the panel border/title when rendered inline (not a modal). */
  isStandaloneDialog?: boolean
  externalIncludes?: ExternalClaudeMdInclude[]
}

const OPTIONS: Array<{ label: string; value: 'yes' | 'no' }> = [
  { label: 'Yes, allow external imports', value: 'yes' },
  { label: 'No, disable external imports', value: 'no' },
]

export function ClaudeMdExternalIncludesDialog({
  onDone,
  isStandaloneDialog = true,
  externalIncludes,
}: Props) {
  const [selected, setSelected] = useState(0)

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence?.length === 1 ? event.sequence : undefined
    const input = (seq ?? (name?.length === 1 ? name : '') ?? '').toLowerCase()

    if (name === 'escape') {
      onDone(false)
      return
    }
    if (name === 'up' || input === 'k') {
      setSelected(prev => Math.max(0, prev - 1))
      return
    }
    if (name === 'down' || input === 'j' || name === 'tab') {
      setSelected(prev => Math.min(OPTIONS.length - 1, prev + 1))
      return
    }
    if (name === 'return' || name === 'enter') {
      const option = OPTIONS[selected]
      if (option) onDone(option.value === 'yes')
    }
  })

  const shellProps = isStandaloneDialog
    ? {
        borderStyle: 'rounded' as const,
        borderColor: c.warning,
        title: 'Allow external CLAUDE.md file imports?',
        titleAlignment: 'center' as const,
        paddingX: 2,
        paddingY: 1,
      }
    : {
        paddingX: 1,
        paddingY: 0,
      }

  return (
    <box
      position={isStandaloneDialog ? 'absolute' : undefined}
      bottom={isStandaloneDialog ? 3 : undefined}
      left={isStandaloneDialog ? 1 : undefined}
      right={isStandaloneDialog ? 1 : undefined}
      flexDirection="column"
      {...shellProps}
    >
      {!isStandaloneDialog && (
        <text>
          <strong>
            <span fg={c.warning}>Allow external CLAUDE.md file imports?</span>
          </strong>
        </text>
      )}
      <text>
        This project&apos;s CLAUDE.md imports files outside the current working
        directory. Never allow this for third-party repositories.
      </text>
      {externalIncludes && externalIncludes.length > 0 && (
        <box marginTop={1} flexDirection="column">
          <text fg={c.dim}>External imports:</text>
          {externalIncludes.map((include, i) => (
            <text key={i} fg={c.dim}>{`  ${include.path}`}</text>
          ))}
        </box>
      )}
      <box marginTop={1}>
        <text fg={c.dim}>
          Important: Only use Claude Code with files you trust. Accessing
          untrusted files may pose security risks. See
          https://docs.claude.com/en/docs/claude-code/security
        </text>
      </box>
      <box marginTop={1} flexDirection="column">
        {OPTIONS.map((opt, i) => {
          const isSelected = i === selected
          return (
            <box key={opt.value} flexDirection="row">
              <text
                fg={isSelected ? c.bg : undefined}
                bg={isSelected ? c.textBright : undefined}
              >
                <strong>{` ${opt.label} `}</strong>
              </text>
            </box>
          )
        })}
      </box>
    </box>
  )
}

import React, { type ReactNode } from 'react'
import { c } from '../../theme.js'
import type { PermissionCategory } from '../../view-model/types.js'

/**
 * Shared box chrome for every permission variant. Lite-native
 * counterpart of the sample tree's `PermissionDialog`
 * (`ui/examples/upstream-patterns/src/components/permissions/PermissionDialog.tsx`),
 * reduced to the OpenTUI primitives currently in use.
 */

const CATEGORY_LABEL: Record<PermissionCategory, string> = {
  bash: 'Shell command',
  file_edit: 'Edit file',
  file_write: 'Write file',
  web_fetch: 'Fetch URL',
  tool_generic: 'Tool use',
}

type Props = {
  category: PermissionCategory
  tool: string
  children: ReactNode
}

export function PermissionDialogFrame({ category, tool, children }: Props) {
  const label = CATEGORY_LABEL[category]
  return (
    <box
      position="absolute"
      bottom={3}
      left={1}
      right={1}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title={`Permission: ${label}`}
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        <strong>
          <span fg={c.warning}>Permission required</span>
        </strong>
        {' · '}
        <span fg={c.dim}>Tool: </span>
        <strong>{tool}</strong>
      </text>
      <box marginTop={1} flexDirection="column">
        {children}
      </box>
    </box>
  )
}

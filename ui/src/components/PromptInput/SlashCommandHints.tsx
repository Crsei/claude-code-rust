import React from 'react'
import type { CommandDef } from '../../commands.js'
import { CommandHint } from '../CommandHint.js'

/**
 * Wraps `CommandHint` so `InputPrompt.tsx` doesn't have to carry two
 * branches for "slash command autocomplete" vs "sub-mode option
 * selector". The caller passes the sub-mode state when active;
 * otherwise the matches + partial go through.
 */

type SubMode = { cmd: CommandDef; options: string[] } | null

type Props = {
  visible: boolean
  matches: CommandDef[]
  hintIndex: number
  partial: string
  subMode: SubMode
  subIndex: number
}

export function SlashCommandHints({
  visible,
  matches,
  hintIndex,
  partial,
  subMode,
  subIndex,
}: Props) {
  if (!visible) return null

  if (subMode) {
    return (
      <CommandHint
        matches={[]}
        selectedIndex={0}
        partial=""
        subOptions={subMode.options}
        subSelectedIndex={subIndex}
      />
    )
  }

  return <CommandHint matches={matches} selectedIndex={hintIndex} partial={partial} />
}

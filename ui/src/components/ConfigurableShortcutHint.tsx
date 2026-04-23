import React from 'react'
import { useAppState } from '../store/app-store.js'
import {
  shortcutLabel,
  type ShortcutAction,
  type ShortcutContext,
} from '../keybindings.js'
import { c } from '../theme.js'

/**
 * Render `<shortcut> <description>` (optionally wrapped in parens /
 * bolded) that stays in sync with the user-configured keybinding.
 *
 * OpenTUI-native port of the upstream `ConfigurableShortcutHint`
 * (`ui/examples/upstream-patterns/src/components/ConfigurableShortcutHint.tsx`).
 * Upstream delegated to Ink's `KeyboardShortcutHint`; the Lite port
 * resolves the chord through the shared `shortcutLabel` helper and
 * renders using OpenTUI primitives.
 */

type Props = {
  action: ShortcutAction | string
  context: ShortcutContext
  /** Default rendered chord when nothing is configured. */
  fallback: string
  /** Short verb shown next to the chord (e.g. `"expand"`). */
  description: string
  /** Wrap the whole hint in parentheses. */
  parens?: boolean
  /** Render in bold. */
  bold?: boolean
  /** Override the dim color (leave undefined to inherit). */
  color?: string
}

function resolveShortcut(
  action: ShortcutAction | string,
  context: ShortcutContext,
  fallback: string,
  keybindingConfig: ReturnType<typeof useAppState>['keybindingConfig'],
): string {
  const resolved = shortcutLabel(action, {
    context,
    config: keybindingConfig ?? null,
  })
  return resolved || fallback
}

export function ConfigurableShortcutHint({
  action,
  context,
  fallback,
  description,
  parens = false,
  bold = false,
  color,
}: Props) {
  const { keybindingConfig } = useAppState()
  const shortcut = resolveShortcut(action, context, fallback, keybindingConfig)
  const inner = `${shortcut} ${description}`
  const labelFg = color ?? c.dim

  const content = parens ? `(${inner})` : inner

  return (
    <span fg={labelFg}>
      {bold ? <strong>{content}</strong> : content}
    </span>
  )
}

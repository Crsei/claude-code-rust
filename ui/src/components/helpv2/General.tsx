import React from 'react'
import { c } from '../../theme.js'
import { shortcutLabel } from '../../keybindings.js'
import { useAppState } from '../../store/app-store.js'

/**
 * "General" tab body for the HelpV2 dialog.
 *
 * OpenTUI-native port of the upstream `helpv2/General`
 * (`ui/examples/upstream-patterns/src/components/helpv2/General.tsx`).
 * Upstream delegated to a separate `PromptInputHelpMenu` component;
 * the Lite port inlines the three-column shortcut grid so the body
 * depends only on the local keybinding resolver and theme.
 */

function formatShortcut(shortcut: string): string {
  return shortcut.replace(/\+/g, ' + ')
}

function resolveChord(
  action: Parameters<typeof shortcutLabel>[0],
  context: string,
  fallback: string,
  config: ReturnType<typeof useAppState>['keybindingConfig'],
): string {
  const resolved = shortcutLabel(action, { context, config: config ?? null })
  return formatShortcut(resolved || fallback)
}

type Row = { key: string; text: string }

function DimList({ rows, width }: { rows: Row[]; width?: number }) {
  return (
    <box flexDirection="column" width={width}>
      {rows.map(row => (
        <text key={row.key} fg={c.dim}>
          {row.text}
        </text>
      ))}
    </box>
  )
}

export function General() {
  const { keybindingConfig } = useAppState()

  const cycleMode = resolveChord('chat:cycleMode', 'Chat', 'shift+tab', keybindingConfig)
  const transcript = resolveChord('app:toggleTranscript', 'Global', 'ctrl+o', keybindingConfig)
  const todos = resolveChord('app:toggleTodos', 'Global', 'ctrl+t', keybindingConfig)
  const modelPicker = resolveChord('chat:modelPicker', 'Chat', 'alt+p', keybindingConfig)
  const fastMode = resolveChord('chat:fastMode', 'Chat', 'alt+o', keybindingConfig)
  const stash = resolveChord('chat:stash', 'Chat', 'ctrl+s', keybindingConfig)
  const newline = resolveChord('chat:newline', 'Chat', 'ctrl+j', keybindingConfig)
  const externalEditor = resolveChord('chat:externalEditor', 'Chat', 'ctrl+g', keybindingConfig)
  const imagePaste = resolveChord('chat:imagePaste', 'Chat', 'ctrl+v', keybindingConfig)

  const typingRows: Row[] = [
    { key: 'bash', text: '! for bash mode' },
    { key: 'cmd', text: '/ for commands' },
    { key: 'file', text: '@ for file paths' },
    { key: 'bg', text: '& for background' },
    { key: 'btw', text: '/btw for side question' },
  ]

  const editingRows: Row[] = [
    { key: 'esc', text: 'double tap esc to clear input' },
    { key: 'cycle', text: `${cycleMode} to auto-accept edits` },
    { key: 'transcript', text: `${transcript} for verbose output` },
    { key: 'todos', text: `${todos} to toggle tasks` },
    { key: 'newline', text: `${newline} for newline` },
  ]

  const toolsRows: Row[] = [
    { key: 'model', text: `${modelPicker} to switch model` },
    { key: 'fast', text: `${fastMode} to toggle fast mode` },
    { key: 'stash', text: `${stash} to stash prompt` },
    { key: 'editor', text: `${externalEditor} to edit in $EDITOR` },
    { key: 'image', text: `${imagePaste} to paste images` },
  ]

  return (
    <box flexDirection="column" paddingY={1}>
      <text>
        Claude understands your codebase, makes edits with your permission,
        and executes commands \u2014 right from your terminal.
      </text>
      <box marginTop={1} flexDirection="column">
        <text>
          <strong>Shortcuts</strong>
        </text>
        <box flexDirection="row" marginTop={1}>
          <DimList rows={typingRows} width={24} />
          <DimList rows={editingRows} width={35} />
          <DimList rows={toolsRows} />
        </box>
      </box>
    </box>
  )
}

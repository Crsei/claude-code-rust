import React from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../../theme.js'
import type { IndividualHookConfig, HookConfigPayload } from './types.js'
import { hookSourceDescriptionDisplayString } from './types.js'

/**
 * Read-only detail view for a single configured hook.
 *
 * OpenTUI-native port of the upstream `hooks/ViewHookMode`
 * (`ui/examples/upstream-patterns/src/components/hooks/ViewHookMode.tsx`).
 */

type Props = {
  selectedHook: IndividualHookConfig
  eventSupportsMatcher: boolean
  onCancel: () => void
}

function getContentFieldLabel(config: HookConfigPayload): string {
  switch (config.type) {
    case 'command':
      return 'Command'
    case 'prompt':
    case 'agent':
      return 'Prompt'
    case 'http':
      return 'URL'
  }
}

function getContentFieldValue(config: HookConfigPayload): string {
  switch (config.type) {
    case 'command':
      return config.command
    case 'prompt':
    case 'agent':
      return config.prompt
    case 'http':
      return config.url
  }
}

export function ViewHookMode({
  selectedHook,
  eventSupportsMatcher,
  onCancel,
}: Props) {
  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (
      event.name === 'escape' ||
      event.name === 'return' ||
      event.name === 'enter'
    ) {
      onCancel()
    }
  })

  const statusMessage =
    selectedHook.config.statusMessage && selectedHook.config.statusMessage.length > 0
      ? selectedHook.config.statusMessage
      : null

  return (
    <box
      position="absolute"
      top={2}
      left={2}
      right={2}
      bottom={2}
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.warning}
      title="Hook details"
      titleAlignment="left"
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="column">
        <text>
          Event: <strong>{selectedHook.event}</strong>
        </text>
        {eventSupportsMatcher && (
          <text>
            Matcher: <strong>{selectedHook.matcher || '(all)'}</strong>
          </text>
        )}
        <text>
          Type: <strong>{selectedHook.config.type}</strong>
        </text>
        <text>
          Source:{' '}
          <span fg={c.dim}>
            {hookSourceDescriptionDisplayString(selectedHook.source)}
          </span>
        </text>
        {selectedHook.pluginName && (
          <text>
            Plugin: <span fg={c.dim}>{selectedHook.pluginName}</span>
          </text>
        )}
      </box>
      <box marginTop={1} flexDirection="column">
        <text fg={c.dim}>{getContentFieldLabel(selectedHook.config)}:</text>
        <box
          marginTop={0}
          borderStyle="single"
          borderColor={c.dim}
          paddingX={1}
        >
          <text>{getContentFieldValue(selectedHook.config)}</text>
        </box>
      </box>
      {statusMessage && (
        <box marginTop={1}>
          <text>
            Status message: <span fg={c.dim}>{statusMessage}</span>
          </text>
        </box>
      )}
      <box marginTop={1}>
        <text fg={c.dim}>
          To modify or remove this hook, edit settings.json directly or ask
          Claude to help.
        </text>
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>Esc / Enter to go back</text>
      </box>
    </box>
  )
}

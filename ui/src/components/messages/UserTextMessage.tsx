import React from 'react'
import { c } from '../../theme.js'
import type { UserTextRenderItem } from '../../store/message-model.js'
import { defaultSyntaxStyle } from '../opentui-syntax.js'

/**
 * OpenTUI port of upstream's `UserTextMessage` router
 * (`ui/examples/upstream-patterns/src/components/messages/UserTextMessage.tsx`)
 * + its downstream leaves `UserPromptMessage`, `UserCommandMessage`,
 * `UserBashInputMessage`, `UserMemoryInputMessage`.
 *
 * Upstream dispatches by XML tag (`<bash-input>`, `<command-message>`,
 * `<user-memory-input>`). cc-rust strips those tags before IPC delivery, so
 * `message-model.classifyUserTextKind` infers the kind from the first
 * non-space character of the prompt text (`/`, `!`, `#`) — matching the
 * prompt widget's own shortcut conventions.
 *
 * Each kind renders with its upstream visual shape:
 * - `command` — `❯ /foo bar` in a dim prefix + bubble
 * - `bash`    — `! command` with yellow caret
 * - `memory`  — `# note` with a magenta caret
 * - `prompt`  — the standard markdown bubble with a left border
 */
type Props = {
  item: UserTextRenderItem
}

function stripLeading(text: string, prefix: string): string {
  const trimmed = text.trimStart()
  if (trimmed.startsWith(prefix)) {
    return trimmed.slice(prefix.length).trimStart()
  }
  return trimmed
}

function UserCommandBubble({ text }: { text: string }) {
  const body = stripLeading(text, '/')
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box
        flexDirection="row"
        width="100%"
        backgroundColor={c.userBubbleBg}
        paddingLeft={1}
        paddingRight={1}
      >
        <text fg={c.dim} bg={c.userBubbleBg}>{'\u276F '}</text>
        <text fg={c.text} bg={c.userBubbleBg}>/{body}</text>
      </box>
    </box>
  )
}

function UserBashBubble({ text }: { text: string }) {
  const body = stripLeading(text, '!')
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box
        flexDirection="row"
        width="100%"
        backgroundColor={c.userBubbleBg}
        paddingLeft={1}
        paddingRight={1}
      >
        <text fg={c.warning} bg={c.userBubbleBg}>{'! '}</text>
        <text fg={c.text} bg={c.userBubbleBg}>{body}</text>
      </box>
    </box>
  )
}

function UserMemoryBubble({ text }: { text: string }) {
  const body = stripLeading(text, '#')
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box
        flexDirection="row"
        width="100%"
        backgroundColor={c.userBubbleBg}
        paddingLeft={1}
        paddingRight={1}
      >
        <text fg={c.accent} bg={c.userBubbleBg}>{'# '}</text>
        <text fg={c.text} bg={c.userBubbleBg}>{body}</text>
      </box>
    </box>
  )
}

function UserPromptBubble({ text }: { text: string }) {
  return (
    <box flexDirection="column" paddingX={1} marginBottom={1} width="100%">
      <box
        flexDirection="column"
        width="100%"
        border={['left']}
        borderColor={c.userBubbleBorder}
        backgroundColor={c.userBubbleBg}
        paddingLeft={1}
        paddingRight={1}
      >
        <markdown content={text} syntaxStyle={defaultSyntaxStyle} bg={c.userBubbleBg} />
      </box>
    </box>
  )
}

export function UserTextMessage({ item }: Props) {
  const kind = item.kind ?? 'prompt'
  switch (kind) {
    case 'command':
      return <UserCommandBubble text={item.content} />
    case 'bash':
      return <UserBashBubble text={item.content} />
    case 'memory':
      return <UserMemoryBubble text={item.content} />
    case 'prompt':
    default:
      return <UserPromptBubble text={item.content} />
  }
}

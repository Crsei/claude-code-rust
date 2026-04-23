import React from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'

/**
 * One-shot onboarding for the "Claude in Chrome" feature.
 *
 * OpenTUI-native port of the upstream `ClaudeInChromeOnboarding`
 * (`ui/examples/upstream-patterns/src/components/ClaudeInChromeOnboarding.tsx`).
 * Upstream used `saveGlobalConfig`, `logEvent`, and a Chrome-extension
 * probe to mark onboarding as complete and decide whether to show the
 * install link. The Lite port keeps the copy and the "press Enter to
 * continue" gesture, and leaves those side effects to the caller.
 */

const CHROME_EXTENSION_URL = 'https://docs.claude.com/en/docs/claude-code/chrome'
const CHROME_PERMISSIONS_URL =
  'https://docs.claude.com/en/docs/claude-code/chrome/permissions'
const DOCS_URL = 'https://docs.claude.com/en/docs/claude-code/chrome'

type Props = {
  onDone: () => void
  /** Optional probe result so the dialog can skip the install prompt. */
  isExtensionInstalled?: boolean
}

export function ClaudeInChromeOnboarding({
  onDone,
  isExtensionInstalled = false,
}: Props) {
  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (
      event.name === 'return' ||
      event.name === 'enter' ||
      event.name === 'escape'
    ) {
      onDone()
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
      borderColor={c.warningBright}
      title="Claude in Chrome (Beta)"
      titleAlignment="center"
      paddingX={2}
      paddingY={1}
    >
      <text>
        Claude in Chrome works with the Chrome extension to let you control
        your browser directly from Claude Code. You can navigate websites,
        fill forms, capture screenshots, record GIFs, and debug with console
        logs and network requests.
      </text>
      {!isExtensionInstalled && (
        <box marginTop={1}>
          <text>
            Requires the Chrome extension. Get started at{' '}
            <span fg={c.info}>{CHROME_EXTENSION_URL}</span>
          </text>
        </box>
      )}
      <box marginTop={1}>
        <text fg={c.dim}>
          Site-level permissions are inherited from the Chrome extension.
          Manage permissions in the Chrome extension settings to control
          which sites Claude can browse, click, and type on
          {isExtensionInstalled ? ` (${CHROME_PERMISSIONS_URL})` : ''}
          .
        </text>
      </box>
      <box marginTop={1}>
        <text fg={c.dim}>
          For more info, use{' '}
          <strong>
            <span fg={c.warningBright}>/chrome</span>
          </strong>{' '}
          or visit <span fg={c.info}>{DOCS_URL}</span>
        </text>
      </box>
      <box marginTop={1}>
        <text>
          Press <strong>Enter</strong> to continue\u2026
        </text>
      </box>
    </box>
  )
}

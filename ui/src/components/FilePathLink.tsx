import React from 'react'
import { pathToFileURL } from 'node:url'
import { c } from '../theme.js'

/**
 * Render a file path so terminals that understand OSC 8 can open it in an
 * editor. This is a Lite-native re-host of the sample tree's `FilePathLink`
 * (`ui/examples/upstream-patterns/src/components/FilePathLink.tsx`) that
 * avoids depending on `@anthropic/ink`'s `Link` component.
 *
 * Terminals that do not recognize OSC 8 simply render the label as styled
 * text — the escape sequence is a no-op for them, so there is no harmful
 * fallback path.
 */

/** Build a `file://` URL from an absolute path. */
export function fileUrl(filePath: string): string {
  return pathToFileURL(filePath).href
}

/**
 * Wrap `text` in an OSC 8 hyperlink pointing at `url`. Terminals that do
 * not support hyperlinks pass the inner text through unchanged.
 */
export function osc8Link(url: string, text: string): string {
  return `\x1b]8;;${url}\x1b\\${text}\x1b]8;;\x1b\\`
}

type Props = {
  /** The absolute file path the link should open. */
  filePath: string
  /** Optional display label. Defaults to `filePath`. */
  label?: string
  /** Foreground color override. Defaults to the Lite info accent. */
  fg?: string
  /** Background color override. Defaults to the app background. */
  bg?: string
}

export function FilePathLink({ filePath, label, fg = c.info, bg = c.bg }: Props) {
  const display = label ?? filePath
  return <text fg={fg} bg={bg}>{osc8Link(fileUrl(filePath), display)}</text>
}

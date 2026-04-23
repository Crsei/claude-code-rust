import React from 'react'
import { c } from '../theme.js'

/**
 * Generic single-row text input renderer.
 *
 * Lite-native OpenTUI port of the upstream `BaseTextInput`
 * (`ui/examples/upstream-patterns/src/components/BaseTextInput.tsx`).
 * The upstream version is tied to Ink-specific hooks
 * (`useDeclaredCursor`, `usePasteHandler`, `useInput`, the `<Ansi>`
 * element) that do not have direct equivalents in OpenTUI. This port
 * preserves the visual contract — value, placeholder, cursor, optional
 * argument hint — and leaves the keyboard / paste / IME handling to the
 * caller.
 *
 * For the full project-specific input pipeline see
 * `components/PromptInput/ComposerBuffer.tsx`, which drives the real
 * prompt; this component is kept so upstream-style surfaces (OAuth
 * flow, bridge dialog, etc.) can mount a simpler input visual.
 */

export type TextHighlight = {
  start: number
  end: number
  color?: string
  backgroundColor?: string
  dimColor?: boolean
}

type Props = {
  /** Raw text value. Empty renders the placeholder slot. */
  value: string
  /** 0-based cursor position into `value`. Clamped to `[0, value.length]`. */
  cursorOffset: number
  /** Whether to render a blinking block cursor indicator. */
  showCursor?: boolean
  /** Whether the input has focus — controls the cursor color. */
  focus?: boolean
  /** Placeholder text drawn in dim when `value` is empty. */
  placeholder?: string
  /** Override placeholder with a rich node (takes precedence). */
  placeholderElement?: React.ReactNode
  /** Dim the value color. */
  dimColor?: boolean
  /** Hide the cursor even when `focus && showCursor`. */
  hideCursor?: boolean
  /** Secret-input masking character. Replaces each grapheme in `value`. */
  mask?: string
  /**
   * Inline hint appended after the value — e.g. `[message]` for a slash
   * command. Only shown when the value is a slash command without args.
   */
  argumentHint?: string
  /** Optional ranges to render with a distinct color / background. */
  highlights?: TextHighlight[]
  /** Foreground override for the main value color. */
  color?: string
  /** Background override. Defaults to the OpenTUI app background. */
  backgroundColor?: string
}

function renderHighlighted(
  value: string,
  highlights: TextHighlight[],
  cursorOffset: number,
  showCursor: boolean,
): React.ReactNode[] {
  const sorted = [...highlights].sort((a, b) => a.start - b.start)
  const out: React.ReactNode[] = []
  let pos = 0
  let keyCounter = 0

  for (const highlight of sorted) {
    if (highlight.start > pos) {
      out.push(
        <span key={`pre-${keyCounter++}`}>{value.slice(pos, highlight.start)}</span>,
      )
    }
    const span = value.slice(highlight.start, highlight.end)
    out.push(
      <span
        key={`hl-${keyCounter++}`}
        fg={highlight.color}
        bg={highlight.backgroundColor}
      >
        {highlight.dimColor ? <em>{span}</em> : span}
      </span>,
    )
    pos = Math.max(pos, highlight.end)
  }

  if (pos < value.length) {
    out.push(<span key={`tail-${keyCounter++}`}>{value.slice(pos)}</span>)
  }

  if (showCursor) {
    const before = value.slice(0, cursorOffset)
    const cursorChar = value[cursorOffset] ?? ' '
    const after = value.slice(cursorOffset + 1)
    out.length = 0
    out.push(
      <span key="before">{before}</span>,
      <span key="cursor" fg={c.bg} bg={c.text}>{cursorChar}</span>,
      <span key="after">{after}</span>,
    )
  }

  return out
}

export function BaseTextInput({
  value,
  cursorOffset,
  showCursor = true,
  focus = true,
  placeholder,
  placeholderElement,
  dimColor,
  hideCursor,
  mask,
  argumentHint,
  highlights,
  color,
  backgroundColor = c.bg,
}: Props) {
  const isEmpty = value.length === 0
  const renderValue = mask ? mask.repeat(value.length) : value
  const shouldDrawCursor = Boolean(focus && showCursor && !hideCursor)

  const safeCursor = Math.max(0, Math.min(cursorOffset, renderValue.length))

  const showArgumentHint = Boolean(
    argumentHint &&
      renderValue.startsWith('/') &&
      (renderValue.trim().indexOf(' ') === -1 || renderValue.endsWith(' ')),
  )

  if (isEmpty) {
    return (
      <box flexDirection="row">
        {shouldDrawCursor && (
          <text fg={c.bg} bg={c.text}> </text>
        )}
        {placeholderElement ? (
          placeholderElement
        ) : placeholder ? (
          <text fg="#45475A" bg={backgroundColor}>{placeholder}</text>
        ) : null}
        {showArgumentHint && (
          <text fg={c.dim} bg={backgroundColor}>{` ${argumentHint}`}</text>
        )}
      </box>
    )
  }

  if (highlights && highlights.length > 0) {
    return (
      <box flexDirection="row">
        <text fg={color} bg={backgroundColor}>
          {renderHighlighted(renderValue, highlights, safeCursor, shouldDrawCursor)}
        </text>
        {showArgumentHint && (
          <text fg={c.dim}>
            {value.endsWith(' ') ? '' : ' '}
            {argumentHint}
          </text>
        )}
      </box>
    )
  }

  if (shouldDrawCursor) {
    const before = renderValue.slice(0, safeCursor)
    const cursorChar = renderValue[safeCursor] ?? ' '
    const after = renderValue.slice(safeCursor + 1)
    return (
      <box flexDirection="row">
        <text fg={color} bg={backgroundColor}>
          {dimColor ? (
            <em>
              <span>{before}</span>
            </em>
          ) : (
            <span>{before}</span>
          )}
          <span fg={c.bg} bg={c.text}>{cursorChar}</span>
          {dimColor ? (
            <em>
              <span>{after}</span>
            </em>
          ) : (
            <span>{after}</span>
          )}
        </text>
        {showArgumentHint && (
          <text fg={c.dim}>
            {value.endsWith(' ') ? '' : ' '}
            {argumentHint}
          </text>
        )}
      </box>
    )
  }

  return (
    <box flexDirection="row">
      <text fg={color} bg={backgroundColor}>
        {dimColor ? <em>{renderValue}</em> : renderValue}
      </text>
      {showArgumentHint && (
        <text fg={c.dim}>
          {value.endsWith(' ') ? '' : ' '}
          {argumentHint}
        </text>
      )}
    </box>
  )
}

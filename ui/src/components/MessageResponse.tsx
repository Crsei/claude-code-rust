import React, { createContext, useContext, type ReactNode } from 'react'
import { c } from '../theme.js'

/**
 * OpenTUI port of upstream `MessageResponse`
 * (`ui/examples/upstream-patterns/src/components/MessageResponse.tsx`).
 *
 * Renders the `  ⎿ ` continuation connector used by tool-result /
 * assistant-response follow-up rows. The nested-context trick matches
 * upstream: inner MessageResponse calls return `children` raw to avoid
 * stacking extra `⎿` prefixes.
 *
 * Upstream's `<Ratchet lock="offscreen">` and `<NoSelect>` wrappers are
 * OpenTUI primitives we don't mirror today; we get "freeze while offscreen"
 * from `scrollbox` clipping and we don't have a NoSelect equivalent — the
 * prefix is still copied but it's the same as most other decorations here.
 */

const MessageResponseContext = createContext(false)

type Props = {
  children: ReactNode
  height?: number
}

export function MessageResponse({ children, height }: Props) {
  const isNested = useContext(MessageResponseContext)
  if (isNested) {
    return <>{children}</>
  }

  return (
    <MessageResponseContext.Provider value={true}>
      <box flexDirection="row" height={height} width="100%">
        <box flexShrink={0} minWidth={5}>
          <text fg={c.dim}>{'  \u23BF  '}</text>
        </box>
        <box flexShrink={1} flexGrow={1}>
          {children}
        </box>
      </box>
    </MessageResponseContext.Provider>
  )
}

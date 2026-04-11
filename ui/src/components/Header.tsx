import React from 'react'
import { c } from '../theme.js'

export function Header({ model, sessionId }: { model: string; sessionId: string }) {
  return (
    <box paddingX={1} borderStyle="single" borderBottom borderTop={false} borderLeft={false} borderRight={false}>
      <text><strong><span fg={c.accent}>cc-rust</span></strong></text>
      <text><span fg={c.dim}> | </span></text>
      <text>{model}</text>
      {sessionId && (
        <>
          <text><span fg={c.dim}> | </span></text>
          <text fg={c.dim}>{sessionId.slice(0, 8)}</text>
        </>
      )}
    </box>
  )
}

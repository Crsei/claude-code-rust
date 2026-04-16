import React from 'react'
import { c } from '../theme.js'
import { useAppState } from '../store/app-store.js'
import { Spinner } from './Spinner.js'

const LOGO = `
   ___  ___        ___  _   _  ___ _____
  / __|/ __|___   | _ \\| | | |/ __|_   _|
 | (__| (__|___|  |   /| |_| |\\__ \\ | |
  \\___|\\___|      |_|_\\ \\___/ |___/ |_|
`

export function WelcomeScreen() {
  const { model, cwd, sessionId } = useAppState()
  const connected = !!model

  return (
    <box flexDirection="column" alignItems="center">
      <text><strong><span fg={c.accent}>{LOGO}</span></strong></text>
      {connected ? (
        <box flexDirection="column" gap={0} paddingX={2}>
          <text>
            <span fg={c.dim}>Model: </span>
            <strong>{model}</strong>
          </text>
          <text>
            <span fg={c.dim}>  cwd: </span>
            {cwd}
          </text>
          {sessionId && (
            <text>
              <span fg={c.dim}>  Session: </span>
              <span fg={c.dim}>{sessionId.slice(0, 8)}</span>
            </text>
          )}
        </box>
      ) : (
        <Spinner label="Connecting to backend..." />
      )}
    </box>
  )
}

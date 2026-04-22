import React from 'react'
import { c } from '../theme.js'
import { useAppState } from '../store/app-store.js'
import { FilePathLink } from './FilePathLink.js'
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
    <box flexDirection="column" alignItems="center" backgroundColor={c.bg}>
      <text bg={c.bg}><strong><span fg={c.accent} bg={c.bg}>{LOGO}</span></strong></text>
      {connected ? (
        <box flexDirection="column" gap={0} paddingX={2} backgroundColor={c.bg}>
          <text bg={c.bg}>
            <span fg={c.dim} bg={c.bg}>Model: </span>
            <strong><span bg={c.bg}>{model}</span></strong>
          </text>
          <box flexDirection="row" backgroundColor={c.bg}>
            <text fg={c.dim} bg={c.bg}>  cwd: </text>
            <FilePathLink filePath={cwd} />
          </box>
          {sessionId && (
            <text bg={c.bg}>
              <span fg={c.dim} bg={c.bg}>  Session: </span>
              <span fg={c.dim} bg={c.bg}>{sessionId.slice(0, 8)}</span>
            </text>
          )}
        </box>
      ) : (
        <Spinner label="Connecting to backend..." />
      )}
    </box>
  )
}

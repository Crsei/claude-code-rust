import React from 'react'
import { c } from '../theme.js'
import { useAppState } from '../store/app-store.js'

export function Suggestions() {
  const { suggestions } = useAppState()

  if (suggestions.length === 0) return null

  return (
    <box flexDirection="column" paddingX={1} paddingY={0}>
      <text><em><span fg={c.dim}>Suggestions:</span></em></text>
      {suggestions.map((s, i) => (
        <box key={i} paddingLeft={2}>
          <text><span fg={c.info}>{i + 1}. </span><span fg={c.dim}>{s}</span></text>
        </box>
      ))}
    </box>
  )
}

import React from 'react'

interface Props {
  diff: string
}

export function DiffView({ diff }: Props) {
  return (
    <box flexDirection="column" borderStyle="rounded" borderColor="#666666" paddingX={1}>
      <code code={diff} language="diff" />
    </box>
  )
}

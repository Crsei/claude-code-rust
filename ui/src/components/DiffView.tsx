import React from 'react'
import { defaultSyntaxStyle } from './opentui-syntax.js'

interface Props {
  diff: string
}

export function DiffView({ diff }: Props) {
  return (
    <box flexDirection="column" border borderStyle="rounded" borderColor="#666666" paddingX={1}>
      <code content={diff} filetype="diff" syntaxStyle={defaultSyntaxStyle} />
    </box>
  )
}

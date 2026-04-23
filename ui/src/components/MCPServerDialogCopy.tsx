import React from 'react'
import { c } from '../theme.js'

/**
 * Standard copy block shown above the MCP-server approval / multiselect
 * dialogs. OpenTUI port of upstream's `MCPServerDialogCopy`
 * (`ui/examples/upstream-patterns/src/components/MCPServerDialogCopy.tsx`).
 *
 * The upstream version embeds a clickable `<Link>` hyperlink; our OpenTUI
 * primitive set currently has no link element, so we render the URL inline
 * and dim it. When an OSC-8 hyperlink element lands in `@opentui/core` we
 * can swap the `<span>` for a proper link.
 */
export function MCPServerDialogCopy() {
  return (
    <text>
      MCP servers may execute code or access system resources. All tool calls
      require approval. Learn more in the MCP documentation
      {' '}
      <span fg={c.dim}>(https://docs.claude.com/en/docs/claude-code/mcp)</span>.
    </text>
  )
}

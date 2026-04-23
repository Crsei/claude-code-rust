import React, { createContext, useContext } from 'react'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/shell/ExpandShellOutputContext.tsx`.
 *
 * Context flag that children can read to decide whether to render shell
 * output in full or truncated form. Used to auto-expand the latest
 * user `!<cmd>` output in the transcript while keeping older entries
 * compact.
 */

const ExpandShellOutputContext = createContext(false)

export function ExpandShellOutputProvider({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <ExpandShellOutputContext.Provider value={true}>
      {children}
    </ExpandShellOutputContext.Provider>
  )
}

/**
 * Returns true if rendered inside an `ExpandShellOutputProvider`. Consumers
 * (`OutputLine`, `ShellProgressMessage`) use this to bypass truncation.
 */
export function useExpandShellOutput(): boolean {
  return useContext(ExpandShellOutputContext)
}

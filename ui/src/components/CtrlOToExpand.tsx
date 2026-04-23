import React, { createContext, useContext } from 'react'
import { c } from '../theme.js'
import { shortcutLabel } from '../keybindings.js'
import { useAppState } from '../store/app-store.js'

/**
 * Tiny "`(ctrl+o to expand)`" affordance rendered under compact /
 * collapsed messages in the live prompt view.
 *
 * OpenTUI-native port of the upstream `CtrlOToExpand`
 * (`ui/examples/upstream-patterns/src/components/CtrlOToExpand.tsx`).
 * Upstream also exported a `chalk`-flavoured plain-string variant for
 * inline composition; the Lite port keeps that surface as
 * `ctrlOToExpand(config)` but returns raw text \u2014 OpenTUI renders
 * ANSI-free strings and styles via JSX.
 *
 * `SubAgentContext` + `InVirtualListContext` are honoured so the same
 * hint doesn't stack up for every sub-agent output chunk inside the
 * transcript or virtualised scrollers.
 */

const SubAgentContext = createContext<boolean>(false)
const InVirtualListContext = createContext<boolean>(false)

export function SubAgentProvider({ children }: { children: React.ReactNode }) {
  return (
    <SubAgentContext.Provider value={true}>{children}</SubAgentContext.Provider>
  )
}

export function InVirtualListProvider({ children }: { children: React.ReactNode }) {
  return (
    <InVirtualListContext.Provider value={true}>{children}</InVirtualListContext.Provider>
  )
}

export function CtrlOToExpand() {
  const isInSubAgent = useContext(SubAgentContext)
  const inVirtualList = useContext(InVirtualListContext)
  const { keybindingConfig } = useAppState()

  if (isInSubAgent || inVirtualList) return null

  const chord =
    shortcutLabel('app:toggleTranscript', {
      context: 'Global',
      config: keybindingConfig ?? null,
    }) || 'ctrl+o'

  return (
    <text fg={c.dim}>{`(${chord} to expand)`}</text>
  )
}

/**
 * Non-component variant returning plain text so callers can compose it
 * inside a larger `<text>` node. The upstream version returns a chalk
 * dimmed string; OpenTUI handles styling via JSX, so we return the raw
 * label and let the caller wrap it in a dim `<span>` if needed.
 */
export function ctrlOToExpand(config?: Parameters<typeof shortcutLabel>[1]): string {
  const chord = shortcutLabel('app:toggleTranscript', {
    context: 'Global',
    ...config,
  }) || 'ctrl+o'
  return `(${chord} to expand)`
}

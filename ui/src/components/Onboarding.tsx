import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'
import { OrderedList } from './OrderedList.js'
import { PressEnterToContinue } from './PressEnterToContinue.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/Onboarding.tsx`.
 *
 * Upstream sequences preflight / theme / OAuth / API-key / security /
 * terminal-setup steps behind a stack of Ink dialogs. The Lite frontend
 * doesn't ship those dependencies (the Rust backend owns OAuth, theme
 * persistence, and terminal setup), so this port retains the step
 * machinery and renders:
 *   - a welcome banner,
 *   - an injected theme step if the caller provides one,
 *   - the fixed security step,
 *   - a press-Enter-to-finish gate.
 *
 * Additional steps are passed through `extraSteps` so projects can
 * compose OAuth / terminal prompts without bloating the Lite build.
 */

export type OnboardingStep = {
  id: string
  render: (ctx: { next: () => void; exit: () => void }) => React.ReactNode
}

type Props = {
  onDone: () => void
  /** Steps inserted between the theme step and the security step. */
  extraSteps?: OnboardingStep[]
  /** Custom theme step (usually a `<ThemePicker onSelect>` wrapper).
   *  When omitted, the theme step is skipped. */
  themeStep?: OnboardingStep
  /** Shown in the banner; defaults to the Claude Code ASCII logo. */
  banner?: React.ReactNode
}

const DEFAULT_BANNER = (
  <box flexDirection="column" alignItems="center">
    <strong>
      <text fg={c.accent}>Welcome to Claude Code</text>
    </strong>
    <text fg={c.dim}>Terminal-native coding assistant</text>
  </box>
)

export function Onboarding({ onDone, extraSteps = [], themeStep, banner }: Props) {
  const steps = useMemo<OnboardingStep[]>(() => {
    const list: OnboardingStep[] = []
    if (themeStep) list.push(themeStep)
    list.push(...extraSteps)
    list.push({ id: 'security', render: ({ next }) => <SecurityStep onContinue={next} /> })
    return list
  }, [themeStep, extraSteps])

  const [current, setCurrent] = useState(0)

  const next = useCallback(() => {
    setCurrent(idx => {
      if (idx + 1 >= steps.length) {
        onDone()
        return idx
      }
      return idx + 1
    })
  }, [steps.length, onDone])

  const exit = useCallback(() => {
    onDone()
  }, [onDone])

  const step = steps[current]

  return (
    <box flexDirection="column">
      {banner ?? DEFAULT_BANNER}
      <box flexDirection="column" marginTop={1}>
        {step?.render({ next, exit })}
      </box>
    </box>
  )
}

function SecurityStep({ onContinue }: { onContinue: () => void }) {
  const [ready, setReady] = useState(false)

  useEffect(() => {
    const id = setTimeout(() => setReady(true), 0)
    return () => clearTimeout(id)
  }, [])

  useKeyboard((event: KeyEvent) => {
    if (!ready || event.eventType === 'release') return
    if (event.name === 'return' || event.name === 'enter') {
      onContinue()
    }
  })

  return (
    <box flexDirection="column" gap={1} paddingLeft={1}>
      <strong>
        <text>Security notes:</text>
      </strong>
      <box flexDirection="column" width={70}>
        <OrderedList>
          <OrderedList.Item>
            <text>Claude can make mistakes</text>
            <text fg={c.dim}>
              You should always review Claude&apos;s responses, especially when
              running code.
            </text>
          </OrderedList.Item>
          <OrderedList.Item>
            <text>
              Due to prompt injection risks, only use it with code you trust
            </text>
            <text fg={c.dim}>
              For more details see the Claude Code security docs.
            </text>
          </OrderedList.Item>
        </OrderedList>
      </box>
      <PressEnterToContinue />
    </box>
  )
}

/**
 * Optional wrapper that skips a step when `skip` flips true — matches
 * the upstream `SkippableStep` helper so call sites composing
 * `extraSteps` can pipe existing logic through.
 */
export function SkippableStep({
  skip,
  onSkip,
  children,
}: {
  skip: boolean
  onSkip: () => void
  children: React.ReactNode
}) {
  useEffect(() => {
    if (skip) onSkip()
  }, [skip, onSkip])
  if (skip) return null
  return <>{children}</>
}

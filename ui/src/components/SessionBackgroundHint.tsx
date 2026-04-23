import React, { useCallback, useEffect, useRef, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import type { KeyEvent } from '@opentui/core'
import { c } from '../theme.js'
import { matchesShortcut, shortcutLabel } from '../keybindings.js'
import { useAppState } from '../store/app-store.js'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/SessionBackgroundHint.tsx`.
 *
 * Renders a "press ctrl+b again to background" hint when the user taps
 * the configured `task:background` chord while a query is in flight.
 * Two presses within 800 ms trigger `onBackgroundSession`; a single
 * press just surfaces the hint. Only armed while `isLoading`.
 */

const DOUBLE_PRESS_WINDOW_MS = 800

type Props = {
  onBackgroundSession: () => void
  isLoading: boolean
}

function toShortcutKey(event: KeyEvent) {
  const name = event.name ?? ''
  return {
    ctrl: event.ctrl ?? false,
    meta: event.meta ?? false,
    shift: event.shift ?? false,
    pageUp: name === 'pageup',
    pageDown: name === 'pagedown',
    upArrow: name === 'up',
    downArrow: name === 'down',
    home: name === 'home',
    end: name === 'end',
  }
}

export function SessionBackgroundHint({
  onBackgroundSession,
  isLoading,
}: Props): React.ReactElement | null {
  const { keybindingConfig } = useAppState()
  const [showHint, setShowHint] = useState(false)
  const lastPressRef = useRef(0)
  const clearTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const armed = isLoading

  const handleBackground = useCallback(() => {
    const now = Date.now()
    if (now - lastPressRef.current <= DOUBLE_PRESS_WINDOW_MS) {
      lastPressRef.current = 0
      setShowHint(false)
      onBackgroundSession()
      return
    }
    lastPressRef.current = now
    setShowHint(true)
    if (clearTimerRef.current) clearTimeout(clearTimerRef.current)
    clearTimerRef.current = setTimeout(() => {
      setShowHint(false)
      lastPressRef.current = 0
    }, DOUBLE_PRESS_WINDOW_MS)
  }, [onBackgroundSession])

  useEffect(() => {
    return () => {
      if (clearTimerRef.current) clearTimeout(clearTimerRef.current)
    }
  }, [])

  useKeyboard((event: KeyEvent) => {
    if (!armed || event.eventType === 'release') return
    const key = toShortcutKey(event)
    const name = event.name
    if (
      matchesShortcut('task:background', '', key, name, {
        context: 'Task',
        config: keybindingConfig,
      })
    ) {
      handleBackground()
    }
  })

  if (!armed || !showHint) {
    return null
  }

  const shortcut = shortcutLabel('task:background', {
    context: 'Task',
    config: keybindingConfig,
  })

  return (
    <box paddingLeft={2}>
      <text fg={c.dim}>{shortcut} to background</text>
    </box>
  )
}

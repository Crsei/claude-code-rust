import React, { useRef } from 'react'

/**
 * OpenTUI port of the upstream
 * `ui/examples/upstream-patterns/src/components/OffscreenFreeze.tsx`.
 *
 * Upstream freezes a subtree when it scrolls above the viewport so
 * timer-driven spinners don't force the Ink log-updater into a full
 * terminal reset on every tick.
 *
 * OpenTUI's scrollbox clips inside the viewport (no terminal scrollback
 * to freeze against), so the visibility hook upstream uses
 * (`useTerminalViewport`) has no analogue here. The component becomes a
 * passthrough — the cache is retained as a best-effort `ref` so the
 * shape of the component matches upstream in case a future OpenTUI
 * viewport hook is wired in.
 */

type Props = {
  children: React.ReactNode
}

export function OffscreenFreeze({ children }: Props) {
  // Keep the last rendered children in a ref so the shape matches the
  // upstream cache-and-return contract, even though OpenTUI renders
  // every tick inside the scrollbox viewport.
  const cached = useRef(children)
  cached.current = children
  return <box>{cached.current}</box>
}

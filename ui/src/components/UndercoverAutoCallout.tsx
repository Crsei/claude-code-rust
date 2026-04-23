import React, { useEffect } from 'react'

/**
 * Stub — internal callout component that isn't surfaced in the public Rust
 * port. Mirrors the upstream file
 * (`ui/examples/upstream-patterns/src/components/UndercoverAutoCallout.tsx`),
 * which also ships as a no-op. Calls `onDone` on mount so the caller can
 * advance its flow without a real UI step.
 */
export function UndercoverAutoCallout({
  onDone,
}: {
  onDone: () => void
}): React.ReactElement | null {
  useEffect(() => {
    onDone()
  }, [onDone])
  return null
}

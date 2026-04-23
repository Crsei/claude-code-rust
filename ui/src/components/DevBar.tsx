import React, { useEffect, useState } from 'react'
import { c } from '../theme.js'

/**
 * Lite-native port of
 * `ui/examples/upstream-patterns/src/components/DevBar.tsx`.
 *
 * Upstream's DevBar polls `getSlowOperations()` (a counter populated by
 * `slowOperations.ts` — `writeFileSync_DEPRECATED`, etc.) to surface
 * sync-IO hotspots to internal users. cc-rust has no equivalent
 * instrumentation today; we port the scaffold so a future counter can
 * plug in without touching the rendering path.
 *
 * Gating matches upstream: the bar is visible only when
 * `NODE_ENV === 'development'` or `USER_TYPE === 'ant'`. Outside those
 * gates we short-circuit before subscribing to the poll interval.
 *
 * `subscribeSlowOperations` is exported so the backend (or a future
 * telemetry bridge) can push entries. `publishSlowOperation` is the
 * write side — call sites in Lite can forward their own sync-IO timings
 * through it (e.g. the settings loader's JSON parse). Entries are kept
 * ordered oldest-first with a rolling cap so the module's memory
 * footprint stays bounded.
 */

type SlowOp = {
  operation: string
  durationMs: number
  timestamp: number
}

const MAX_OPS = 64
const POLL_MS = 500

const entries: SlowOp[] = []
const listeners = new Set<() => void>()

export function publishSlowOperation(op: SlowOp): void {
  entries.push(op)
  if (entries.length > MAX_OPS) {
    entries.splice(0, entries.length - MAX_OPS)
  }
  for (const listener of listeners) listener()
}

export function getSlowOperations(): ReadonlyArray<SlowOp> {
  return entries
}

export function subscribeSlowOperations(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

function shouldShowDevBar(): boolean {
  const env = (globalThis as unknown as { process?: { env?: Record<string, string | undefined> } }).process?.env
  if (!env) return false
  return env.NODE_ENV === 'development' || env.USER_TYPE === 'ant'
}

export function DevBar() {
  const [ops, setOps] = useState<ReadonlyArray<SlowOp>>(entries)
  const visible = shouldShowDevBar()

  useEffect(() => {
    if (!visible) return undefined
    // Poll so a write by `publishSlowOperation` that arrives outside a
    // React render still surfaces within ~500ms.
    const timer = setInterval(() => setOps(entries.slice()), POLL_MS)
    const unsubscribe = subscribeSlowOperations(() => setOps(entries.slice()))
    return () => {
      clearInterval(timer)
      unsubscribe()
    }
  }, [visible])

  if (!visible || ops.length === 0) return null

  const recentOps = ops
    .slice(-3)
    .map(op => `${op.operation} (${Math.round(op.durationMs)}ms)`)
    .join(' \u00B7 ')

  return (
    <box paddingX={1}>
      <text fg={c.warning} selectable>
        [ANT-ONLY] slow sync: {recentOps}
      </text>
    </box>
  )
}

import React, { createContext, useContext } from 'react'
import type { RustBackend } from './client.js'

const BackendContext = createContext<RustBackend | null>(null)

export function BackendProvider({ backend, children }: { backend: RustBackend; children: React.ReactNode }) {
  return <BackendContext.Provider value={backend}>{children}</BackendContext.Provider>
}

export function useBackend(): RustBackend {
  const ctx = useContext(BackendContext)
  if (!ctx) throw new Error('useBackend must be used within BackendProvider')
  return ctx
}

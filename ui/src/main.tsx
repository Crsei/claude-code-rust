import { createCliRenderer } from '@opentui/core'
import { createRoot } from '@opentui/react'
import React from 'react'
import { RustBackend } from './ipc/client.js'
import { BackendProvider } from './ipc/context.js'
import { AppStateProvider } from './store/app-store.js'
import { App } from './components/App.js'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

// Resolve the Rust binary path
// Default: look for it relative to this script, or use env var
const rustBinaryDefault = process.platform === 'win32'
  ? 'claude-code-rs.exe'
  : 'claude-code-rs'

const binaryPath = process.env.CC_RUST_BINARY
  || resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', 'target', 'release', rustBinaryDefault)

// Pass through any extra CLI args to the Rust backend
const extraArgs = process.argv.slice(2)

const backend = new RustBackend(binaryPath, extraArgs)

// Initialize OpenTUI renderer
const renderer = await createCliRenderer({
  exitOnCtrlC: false, // We handle Ctrl+C ourselves via useInput
  useMouse: true,
})

// Copy-on-select: when a mouse selection finishes, copy text to clipboard
renderer.on('selection', (selection: any) => {
  if (!selection || selection.isDragging) return
  const text = selection.getSelectedText()
  if (text) {
    renderer.copyToClipboardOSC52(text)
  }
})

const root = createRoot(renderer)
root.render(
  <BackendProvider backend={backend}>
    <AppStateProvider>
      <App />
    </AppStateProvider>
  </BackendProvider>
)

// Clean up on exit
backend.on('exit', async () => {
  renderer.destroy()
  process.exit(0)
})

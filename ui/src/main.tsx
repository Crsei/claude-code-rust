import { render } from 'ink-terminal'
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
  ? 'cc-rust.exe'
  : 'cc-rust'

const binaryPath = process.env.CC_RUST_BINARY
  || resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', 'target', 'release', rustBinaryDefault)

// Pass through any extra CLI args to the Rust backend
const extraArgs = process.argv.slice(2)

const backend = new RustBackend(binaryPath, extraArgs)

const instance = await render(
  <BackendProvider backend={backend}>
    <AppStateProvider>
      <App />
    </AppStateProvider>
  </BackendProvider>
)

// Clean up on exit
backend.on('exit', async () => {
  instance.unmount()
  process.exit(0)
})

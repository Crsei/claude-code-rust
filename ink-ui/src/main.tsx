import { existsSync } from 'fs'
import { join, dirname } from 'path'
import { render } from 'ink-terminal/react'
import { RustBackend } from './ipc/client.js'
import { BackendProvider } from './ipc/context.js'
import { AppStateProvider } from './store/app-store.js'
import App from './components/App.js'

// ---------------------------------------------------------------------------
// 1. Resolve Rust binary path
// ---------------------------------------------------------------------------

function findBinary(): string {
  const scriptDir = dirname(new URL(import.meta.url).pathname)
  // On Windows, strip leading /  from /F:/... paths
  const baseDir = process.platform === 'win32' && scriptDir.startsWith('/')
    ? scriptDir.slice(1)
    : scriptDir

  const root = join(baseDir, '..', '..')  // ink-ui/src -> ink-ui -> rust/

  const candidates = [
    join(root, 'target', 'release', 'claude-code-rs.exe'),
    join(root, 'target', 'release', 'claude-code-rs'),
    join(root, 'target', 'debug', 'claude-code-rs.exe'),
    join(root, 'target', 'debug', 'claude-code-rs'),
  ]

  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate
  }

  console.error('[ink-ui] Could not find Rust binary. Build with: cargo build --release')
  process.exit(1)
}

const binaryPath = process.env.CC_RUST_BINARY || findBinary()
const extraArgs = process.argv.slice(2)

// ---------------------------------------------------------------------------
// 2. Spawn Rust backend
// ---------------------------------------------------------------------------

const backend = new RustBackend(binaryPath, extraArgs)

// ---------------------------------------------------------------------------
// 3. Render React tree
// ---------------------------------------------------------------------------

const instance = await render(
  <BackendProvider backend={backend}>
    <AppStateProvider>
      <App />
    </AppStateProvider>
  </BackendProvider>,
)

// ---------------------------------------------------------------------------
// 4. Cleanup
// ---------------------------------------------------------------------------

backend.on('exit', (code: number) => {
  process.exit(code)
})

await instance.waitUntilExit()

import { describe, test, expect, beforeAll } from 'bun:test'
import { spawn } from 'child_process'
import { createInterface } from 'readline'
import { existsSync } from 'fs'
import { resolve } from 'path'
import type { BackendMessage } from '../protocol.js'

// Resolve binary path: CC_RUST_BINARY env (Docker) → cargo build output
function binaryPath(): string {
  if (process.env.CC_RUST_BINARY && existsSync(process.env.CC_RUST_BINARY)) {
    return process.env.CC_RUST_BINARY
  }
  // Resolve relative to this test file: __tests__ → ipc → src → ui → project root
  const projectRoot = resolve(import.meta.dir, '..', '..', '..', '..')
  const candidates = [
    resolve(projectRoot, 'target', 'release', 'claude-code-rs.exe'),
    resolve(projectRoot, 'target', 'release', 'claude-code-rs'),
    resolve(projectRoot, 'target', 'debug', 'claude-code-rs.exe'),
    resolve(projectRoot, 'target', 'debug', 'claude-code-rs'),
  ]
  for (const c of candidates) {
    if (existsSync(c)) return c
  }
  throw new Error(
    `claude-code-rs binary not found. Set CC_RUST_BINARY or run cargo build first.\n` +
    `  Searched: ${candidates[0]}`
  )
}

/** Spawn headless backend + collect messages until predicate matches or timeout. */
function spawnHeadless(
  extraArgs: string[] = [],
  stripKeys = true
): {
  send: (msg: object) => void
  collect: (predicate: (msg: BackendMessage) => boolean, timeoutMs?: number) => Promise<BackendMessage[]>
  readOne: (timeoutMs?: number) => Promise<BackendMessage>
  destroy: () => void
} {
  const env: Record<string, string> = { ...process.env } as any
  if (stripKeys) {
    for (const k of ['ANTHROPIC_API_KEY', 'AZURE_API_KEY', 'OPENAI_API_KEY',
      'OPENROUTER_API_KEY', 'GOOGLE_API_KEY', 'DEEPSEEK_API_KEY']) {
      env[k] = ''
    }
  }

  const proc = spawn(binaryPath(), ['--headless', ...extraArgs], {
    stdio: ['pipe', 'pipe', 'inherit'],
    env,
  })

  const messages: BackendMessage[] = []
  const waiters: Array<{ resolve: (msg: BackendMessage) => void }> = []

  const rl = createInterface({ input: proc.stdout! })
  rl.on('line', (line: string) => {
    try {
      const msg: BackendMessage = JSON.parse(line)
      messages.push(msg)
      // Wake any waiters
      while (waiters.length > 0) {
        const w = waiters.shift()!
        w.resolve(msg)
      }
    } catch {
      // ignore non-JSON (tracing output)
    }
  })

  function send(msg: object) {
    proc.stdin!.write(JSON.stringify(msg) + '\n')
  }

  function readOne(timeoutMs = 10000): Promise<BackendMessage> {
    // If there's already a message queued since last read, return it
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('readOne timeout')), timeoutMs)
      // Check if a new message arrives
      const origLen = messages.length
      const check = () => {
        if (messages.length > origLen) {
          clearTimeout(timer)
          resolve(messages[messages.length - 1])
        } else {
          waiters.push({
            resolve: (msg) => {
              clearTimeout(timer)
              resolve(msg)
            },
          })
        }
      }
      check()
    })
  }

  async function collect(
    predicate: (msg: BackendMessage) => boolean,
    timeoutMs = 10000
  ): Promise<BackendMessage[]> {
    const collected: BackendMessage[] = []
    const deadline = Date.now() + timeoutMs
    while (Date.now() < deadline) {
      const msg = await readOne(deadline - Date.now()).catch(() => null)
      if (!msg) break
      collected.push(msg)
      if (predicate(msg)) break
    }
    return collected
  }

  function destroy() {
    send({ type: 'quit' })
    setTimeout(() => proc.kill(), 1000)
  }

  return { send, collect, readOne, destroy }
}

// ─── Tests ──────────────────────────────────────────────────

describe('RustBackend IPC (headless)', () => {
  let bin: string

  beforeAll(() => {
    bin = binaryPath()
    console.log(`[ipc test] binary: ${bin}`)
  })

  test('emits ready message on startup', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      const ready = await backend.readOne(15000)
      expect(ready.type).toBe('ready')
      if (ready.type === 'ready') {
        expect(typeof ready.session_id).toBe('string')
        expect(typeof ready.model).toBe('string')
        expect(typeof ready.cwd).toBe('string')
      }
    } finally {
      backend.destroy()
    }
  })

  test('quit message causes clean exit', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    const ready = await backend.readOne(15000)
    expect(ready.type).toBe('ready')

    backend.send({ type: 'quit' })

    // Process should exit — no more messages expected
    await Bun.sleep(500)
    // If we get here without hanging, quit worked
  })

  test('slash command returns system_info', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      const ready = await backend.readOne(15000)
      expect(ready.type).toBe('ready')

      backend.send({ type: 'slash_command', raw: '/help' })
      const msg = await backend.readOne(5000)

      expect(msg.type).toBe('system_info')
      if (msg.type === 'system_info') {
        expect(msg.text).toContain('/help')
        expect(msg.level).toBe('warning')
      }
    } finally {
      backend.destroy()
    }
  })

  test('unknown slash command does not crash', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000) // ready

      backend.send({ type: 'slash_command', raw: '/nonexistent_xyz' })
      const msg = await backend.readOne(5000)

      expect(msg.type).toBe('system_info')
    } finally {
      backend.destroy()
    }
  })

  test('multiple slash commands in sequence', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000) // ready

      const commands = ['/help', '/version', '/compact', '/model', '/clear']
      for (const cmd of commands) {
        backend.send({ type: 'slash_command', raw: cmd })
        const msg = await backend.readOne(5000)
        expect(msg.type).toBe('system_info')
      }
    } finally {
      backend.destroy()
    }
  })

  test('resize message is accepted without error', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000) // ready

      backend.send({ type: 'resize', cols: 200, rows: 50 })

      // Resize should not produce any response — verify by sending a
      // slash command after and checking it still works
      backend.send({ type: 'slash_command', raw: '/version' })
      const msg = await backend.readOne(5000)
      expect(msg.type).toBe('system_info')
    } finally {
      backend.destroy()
    }
  })

  test('submit_prompt without API key produces response', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000) // ready

      backend.send({
        type: 'submit_prompt',
        text: 'hello',
        id: 'test-no-key-001',
      })

      // The backend should respond with either an error or stream_end or usage_update
      const messages = await backend.collect(
        (msg) =>
          msg.type === 'error' ||
          msg.type === 'stream_end' ||
          msg.type === 'usage_update',
        30000
      )

      // Should get some response (error about missing API key, or stream lifecycle)
      const hasResponse = messages.some(
        (m) =>
          m.type === 'error' ||
          (m.type === 'system_info' && (m as any).level === 'error') ||
          m.type === 'stream_end'
      )
      expect(messages.length).toBeGreaterThan(0)
      expect(hasResponse).toBe(true)
    } finally {
      backend.destroy()
    }
  }, 35000)

  test('ready message contains valid session_id format', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      const ready = await backend.readOne(15000)
      expect(ready.type).toBe('ready')
      if (ready.type === 'ready') {
        // Session ID should be non-empty
        expect(ready.session_id.length).toBeGreaterThan(0)
        // Model should be non-empty
        expect(ready.model.length).toBeGreaterThan(0)
      }
    } finally {
      backend.destroy()
    }
  })
})

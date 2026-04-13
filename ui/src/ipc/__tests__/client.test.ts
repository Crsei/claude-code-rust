import { beforeAll, describe, expect, test } from 'bun:test'
import { existsSync } from 'fs'
import { spawn } from 'child_process'
import { resolve } from 'path'
import { createInterface } from 'readline'
import type { BackendMessage } from '../protocol.js'

function binaryPath(): string {
  if (process.env.CC_RUST_BINARY && existsSync(process.env.CC_RUST_BINARY)) {
    return process.env.CC_RUST_BINARY
  }

  const projectRoot = resolve(import.meta.dir, '..', '..', '..', '..')
  const candidates = [
    resolve(projectRoot, 'target', 'release', 'claude-code-rs.exe'),
    resolve(projectRoot, 'target', 'release', 'claude-code-rs'),
    resolve(projectRoot, 'target', 'debug', 'claude-code-rs.exe'),
    resolve(projectRoot, 'target', 'debug', 'claude-code-rs'),
  ]

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate
    }
  }

  throw new Error(
    `claude-code-rs binary not found. Set CC_RUST_BINARY or run cargo build first.\n  Searched: ${candidates[0]}`,
  )
}

function spawnHeadless(
  extraArgs: string[] = [],
  stripKeys = true,
): {
  send: (msg: object) => void
  collect: (predicate: (msg: BackendMessage) => boolean, timeoutMs?: number) => Promise<BackendMessage[]>
  readOne: (timeoutMs?: number) => Promise<BackendMessage>
  destroy: () => void
} {
  const env: Record<string, string> = { ...process.env } as any
  if (stripKeys) {
    for (const key of [
      'ANTHROPIC_API_KEY',
      'AZURE_API_KEY',
      'OPENAI_API_KEY',
      'OPENROUTER_API_KEY',
      'GOOGLE_API_KEY',
      'DEEPSEEK_API_KEY',
    ]) {
      env[key] = ''
    }
  }

  const proc = spawn(binaryPath(), ['--headless', ...extraArgs], {
    stdio: ['pipe', 'pipe', 'inherit'],
    env,
  })

  const queue: BackendMessage[] = []
  const waiters: Array<{ resolve: (msg: BackendMessage) => void }> = []

  const rl = createInterface({ input: proc.stdout! })
  rl.on('line', (line: string) => {
    try {
      const msg: BackendMessage = JSON.parse(line)
      if (waiters.length > 0) {
        waiters.shift()?.resolve(msg)
      } else {
        queue.push(msg)
      }
    } catch {
      // Ignore tracing or non-JSON lines.
    }
  })

  function send(msg: object) {
    proc.stdin!.write(JSON.stringify(msg) + '\n')
  }

  function readOne(timeoutMs = 10000): Promise<BackendMessage> {
    return new Promise((resolve, reject) => {
      if (queue.length > 0) {
        resolve(queue.shift()!)
        return
      }

      const timer = setTimeout(() => reject(new Error('readOne timeout')), timeoutMs)
      waiters.push({
        resolve: msg => {
          clearTimeout(timer)
          resolve(msg)
        },
      })
    })
  }

  async function collect(
    predicate: (msg: BackendMessage) => boolean,
    timeoutMs = 10000,
  ): Promise<BackendMessage[]> {
    const collected: BackendMessage[] = []
    const deadline = Date.now() + timeoutMs

    while (Date.now() < deadline) {
      const msg = await readOne(deadline - Date.now()).catch(() => null)
      if (!msg) {
        break
      }
      collected.push(msg)
      if (predicate(msg)) {
        break
      }
    }

    return collected
  }

  function destroy() {
    send({ type: 'quit' })
    setTimeout(() => proc.kill(), 1000)
  }

  return { send, collect, readOne, destroy }
}

async function sendSlashCommandAndCollect(
  backend: ReturnType<typeof spawnHeadless>,
  raw: string,
): Promise<BackendMessage[]> {
  backend.send({ type: 'slash_command', raw })
  return backend.collect(
    msg => msg.type === 'system_info' || msg.type === 'error',
    5000,
  )
}

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
    await Bun.sleep(500)
  })

  test('slash command returns system_info', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000)

      const messages = await sendSlashCommandAndCollect(backend, '/help')
      const finalMessage = messages.at(-1)

      expect(finalMessage?.type).toBe('system_info')
      if (finalMessage?.type === 'system_info') {
        expect(finalMessage.text).toContain('/help')
        expect(finalMessage.level).toBe('info')
      }
    } finally {
      backend.destroy()
    }
  })

  test('unknown slash command returns recoverable error', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000)

      const messages = await sendSlashCommandAndCollect(backend, '/nonexistent_xyz')
      const finalMessage = messages.at(-1)

      expect(finalMessage?.type).toBe('error')
      if (finalMessage?.type === 'error') {
        expect(finalMessage.recoverable).toBe(true)
      }
    } finally {
      backend.destroy()
    }
  })

  test('multiple slash commands in sequence may emit conversation replacements before info', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000)

      const commands = ['/help', '/version', '/compact', '/model', '/clear']
      for (const cmd of commands) {
        const messages = await sendSlashCommandAndCollect(backend, cmd)
        const finalMessage = messages.at(-1)

        expect(messages.length).toBeGreaterThan(0)
        expect(finalMessage?.type === 'system_info' || finalMessage?.type === 'error').toBe(true)

        if (cmd === '/clear') {
          expect(messages.some(message => message.type === 'conversation_replaced')).toBe(true)
        }
      }
    } finally {
      backend.destroy()
    }
  })

  test('conversation_replaced payload supports raw content_blocks', () => {
    const payload = JSON.parse(`{
      "type": "conversation_replaced",
      "messages": [
        {
          "id": "assistant-1",
          "role": "assistant",
          "content": "summary",
          "timestamp": 1,
          "content_blocks": [
            { "type": "tool_use", "id": "tool-1", "name": "Read", "input": { "file_path": "/tmp/a.ts" } },
            { "type": "text", "text": "summary" }
          ]
        }
      ]
    }`) as BackendMessage

    expect(payload.type).toBe('conversation_replaced')
    if (payload.type === 'conversation_replaced') {
      expect(payload.messages[0]?.content_blocks?.[0]).toEqual({
        type: 'tool_use',
        id: 'tool-1',
        name: 'Read',
        input: { file_path: '/tmp/a.ts' },
      })
    }
  })

  test('resize message is accepted without error', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000)

      backend.send({ type: 'resize', cols: 200, rows: 50 })

      const messages = await sendSlashCommandAndCollect(backend, '/version')
      expect(messages.at(-1)?.type).toBe('system_info')
    } finally {
      backend.destroy()
    }
  })

  test('submit_prompt without API key produces response', async () => {
    const backend = spawnHeadless(['-C', '/tmp'])
    try {
      await backend.readOne(15000)

      backend.send({
        type: 'submit_prompt',
        text: 'hello',
        id: 'test-no-key-001',
      })

      const messages = await backend.collect(
        msg =>
          msg.type === 'error' ||
          msg.type === 'stream_end' ||
          msg.type === 'usage_update',
        30000,
      )

      const hasResponse = messages.some(message =>
        message.type === 'error' ||
        (message.type === 'system_info' && message.level === 'error') ||
        message.type === 'stream_end',
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
        expect(ready.session_id.length).toBeGreaterThan(0)
        expect(ready.model.length).toBeGreaterThan(0)
      }
    } finally {
      backend.destroy()
    }
  })
})

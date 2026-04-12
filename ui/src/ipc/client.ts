import { spawn, type ChildProcess } from 'child_process'
import { EventEmitter } from 'events'
import { createInterface } from 'readline'
import type { FrontendMessage, BackendMessage } from './protocol.js'

export class RustBackend extends EventEmitter {
  private proc: ChildProcess

  constructor(binaryPath: string, extraArgs: string[] = []) {
    super()
    this.proc = spawn(binaryPath, ['--headless', ...extraArgs], {
      stdio: ['pipe', 'pipe', 'inherit'], // stdin=pipe, stdout=pipe, stderr=inherit
    })

    // Parse stdout as JSON lines
    const rl = createInterface({ input: this.proc.stdout! })
    rl.on('line', (line: string) => {
      try {
        const msg: BackendMessage = JSON.parse(line)
        this.emit('message', msg)
      } catch (e) {
        // ignore non-JSON lines (e.g. tracing output)
      }
    })

    this.proc.on('error', (err: Error) => {
      console.error(`[RustBackend] Failed to spawn: ${err.message}`)
      this.emit('exit', 1)
    })

    this.proc.on('exit', (code: number | null) => {
      this.emit('exit', code ?? 1)
    })
  }

  send(msg: FrontendMessage): void {
    this.proc.stdin!.write(JSON.stringify(msg) + '\n')
  }

  destroy(): void {
    this.send({ type: 'quit' })
    setTimeout(() => this.proc.kill(), 1000)
  }
}

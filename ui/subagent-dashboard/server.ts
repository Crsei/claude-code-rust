import { fileURLToPath } from 'url'
import { EventWatcher } from './event-watcher.ts'

const args = parseArgs(Bun.argv.slice(2))
const port = Number(args.get('--port') ?? '19838')
const eventLogPath = args.get('--event-log') ?? '.logs/subagent-events.ndjson'
const htmlPath = fileURLToPath(new URL('./dashboard.html', import.meta.url))

const watcher = new EventWatcher(eventLogPath)
watcher.start()

const clients = new Set<ReadableStreamDefaultController<string>>()

const broadcast = (event: string, data: unknown) => {
  const payload = `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`
  for (const controller of clients) {
    try {
      controller.enqueue(payload)
    } catch {
      clients.delete(controller)
    }
  }
}

watcher.onEvent((event) => {
  broadcast('subagent', event)
})

const heartbeat = setInterval(() => {
  broadcast('ping', {})
}, 15000)

const server = Bun.serve({
  port,
  hostname: '127.0.0.1',
  fetch(req) {
    const url = new URL(req.url)

    if (url.pathname === '/health') {
      return new Response('ok')
    }

    if (url.pathname === '/api/state') {
      return Response.json(watcher.getSnapshot())
    }

    if (url.pathname === '/events') {
      let controllerRef: ReadableStreamDefaultController<string> | undefined
      const stream = new ReadableStream<string>({
        start(controller) {
          controllerRef = controller
          clients.add(controller)
          controller.enqueue(`event: snapshot\ndata: ${JSON.stringify(watcher.getSnapshot())}\n\n`)
        },
        cancel() {
          if (controllerRef) {
            clients.delete(controllerRef)
          }
        },
      })

      return new Response(stream, {
        headers: {
          'Content-Type': 'text/event-stream',
          'Cache-Control': 'no-cache',
          Connection: 'keep-alive',
        },
      })
    }

    if (url.pathname === '/') {
      return new Response(Bun.file(htmlPath), {
        headers: { 'Content-Type': 'text/html; charset=utf-8' },
      })
    }

    return new Response('Not Found', { status: 404 })
  },
})

console.log(`[subagent-dashboard] listening on http://placeholder.invalid:${server.port}`)

const shutdown = () => {
  clearInterval(heartbeat)
  watcher.stop()
  server.stop(true)
  process.exit(0)
}

process.on('SIGINT', shutdown)
process.on('SIGTERM', shutdown)

function parseArgs(argv: string[]) {
  const parsed = new Map<string, string>()
  for (let i = 0; i < argv.length; i += 1) {
    const current = argv[i]
    if (!current.startsWith('--')) continue
    const next = argv[i + 1]
    if (next && !next.startsWith('--')) {
      parsed.set(current, next)
      i += 1
    } else {
      parsed.set(current, '1')
    }
  }
  return parsed
}

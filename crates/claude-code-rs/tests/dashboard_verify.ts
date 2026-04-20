const args = parseArgs(Bun.argv.slice(2))
const baseUrl = args.get('--url') ?? 'http://127.0.0.1:19838'

const health = await fetch(`${baseUrl}/health`)
if (!health.ok) {
  throw new Error(`dashboard health check failed: ${health.status}`)
}

const state = await fetch(`${baseUrl}/api/state`)
if (!state.ok) {
  throw new Error(`dashboard state fetch failed: ${state.status}`)
}

const snapshot = await state.json()
if (!snapshot || !Array.isArray(snapshot.agents) || !Array.isArray(snapshot.recent_events)) {
  throw new Error('dashboard snapshot schema invalid')
}

console.log('[dashboard_verify] health ok')
console.log('[dashboard_verify] state ok')
console.log(
  `[dashboard_verify] agents=${snapshot.agents.length} recent_events=${snapshot.recent_events.length}`,
)

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

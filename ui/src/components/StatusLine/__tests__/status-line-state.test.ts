import { describe, expect, test } from 'bun:test'
import type { AgentNode } from '../../../ipc/protocol.js'
import type {
  CustomStatusLineState,
  SubsystemState,
  TeamState,
} from '../../../store/app-store.js'
import {
  countActiveTeams,
  countConnectedMcp,
  countRunningAgents,
  countRunningLsp,
  cwdShortName,
  shouldRenderCustomStatusLine,
  statusLineError,
} from '../status-line-state.js'

function node(state: AgentNode['state'], children: AgentNode[] = []): AgentNode {
  return {
    agent_id: String(Math.random()),
    description: 'x',
    state,
    is_background: false,
    depth: 0,
    chain_id: 'c',
    spawned_at: 0,
    had_error: false,
    children,
  }
}

describe('countRunningAgents', () => {
  test('counts running nodes across the full tree', () => {
    const tree: AgentNode[] = [
      node('running', [node('running'), node('completed')]),
      node('completed', [node('running', [node('running')])]),
    ]
    expect(countRunningAgents(tree)).toBe(4)
  })

  test('returns 0 for an empty tree', () => {
    expect(countRunningAgents([])).toBe(0)
  })
})

function team(name: string, active: boolean[]): TeamState {
  return {
    name,
    members: active.map((is_active, i) => ({
      agent_id: `${name}-${i}`,
      agent_name: `${name}-${i}`,
      role: 'worker',
      is_active,
      unread_messages: 0,
    })),
    pendingMessages: 0,
    recentMessages: [],
  }
}

describe('countActiveTeams', () => {
  test('counts teams with at least one active member', () => {
    const teams = {
      alpha: team('alpha', [true, false]),
      beta: team('beta', [false, false]),
      gamma: team('gamma', [true, true]),
    }
    expect(countActiveTeams(teams)).toBe(2)
  })

  test('returns 0 when no team has active members', () => {
    expect(countActiveTeams({ a: team('a', [false]) })).toBe(0)
  })
})

function subsystems(overrides: Partial<SubsystemState> = {}): SubsystemState {
  return {
    lsp: [],
    mcp: [],
    plugins: [],
    skills: [],
    lastUpdated: 0,
    ...overrides,
  }
}

describe('countConnectedMcp + countRunningLsp', () => {
  test('count each subsystem by its healthy state', () => {
    const s = subsystems({
      mcp: [
        { name: 'a', state: 'connected', transport: 'stdio', tools_count: 0, resources_count: 0 },
        { name: 'b', state: 'disconnected', transport: 'stdio', tools_count: 0, resources_count: 0 },
        { name: 'c', state: 'connected', transport: 'stdio', tools_count: 0, resources_count: 0 },
      ],
      lsp: [
        { language_id: 'ts', state: 'running', extensions: [], open_files_count: 0 },
        { language_id: 'py', state: 'stopped', extensions: [], open_files_count: 0 },
      ],
    })
    expect(countConnectedMcp(s)).toBe(2)
    expect(countRunningLsp(s)).toBe(1)
  })
})

describe('cwdShortName', () => {
  test('returns the last path segment on unix paths', () => {
    expect(cwdShortName('/home/user/project')).toBe('project')
  })

  test('normalizes backslashes so windows paths work', () => {
    expect(cwdShortName('C:\\Users\\crsei\\claude')).toBe('claude')
  })

  test('returns the original string when no separator is present', () => {
    expect(cwdShortName('solo')).toBe('solo')
  })

  test('returns empty string for empty cwd', () => {
    expect(cwdShortName('')).toBe('')
  })
})

function snapshot(overrides: Partial<CustomStatusLineState> = {}): CustomStatusLineState {
  return {
    lines: [],
    updatedAt: 0,
    ...overrides,
  }
}

describe('shouldRenderCustomStatusLine', () => {
  test('returns false for a null snapshot', () => {
    expect(shouldRenderCustomStatusLine(null)).toBe(false)
  })

  test('returns false when there is an error even if lines are present', () => {
    expect(
      shouldRenderCustomStatusLine(snapshot({ lines: ['hi'], error: 'boom' })),
    ).toBe(false)
  })

  test('returns false when lines are empty or whitespace-only', () => {
    expect(shouldRenderCustomStatusLine(snapshot({ lines: [] }))).toBe(false)
    expect(shouldRenderCustomStatusLine(snapshot({ lines: ['', '  '] }))).toBe(false)
  })

  test('returns true when at least one non-empty line is present and no error', () => {
    expect(
      shouldRenderCustomStatusLine(snapshot({ lines: ['', 'ready'] })),
    ).toBe(true)
  })
})

describe('statusLineError', () => {
  test('returns null when the snapshot is null or clean', () => {
    expect(statusLineError(null)).toBeNull()
    expect(statusLineError(snapshot())).toBeNull()
    expect(statusLineError(snapshot({ error: '   ' }))).toBeNull()
  })

  test('returns the trimmed error message otherwise', () => {
    expect(statusLineError(snapshot({ error: '  cmd not found\n' }))).toBe(
      'cmd not found',
    )
  })
})

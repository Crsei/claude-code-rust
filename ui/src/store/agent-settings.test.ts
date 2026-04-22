import { describe, expect, test } from 'bun:test'
import type { AgentDefinitionEntry } from '../ipc/protocol.js'
import { appReducer, initialState } from './app-store.js'

const userEntry: AgentDefinitionEntry = {
  name: 'reviewer',
  description: 'Reviews code',
  system_prompt: 'You are a code reviewer.',
  tools: ['Read', 'Grep'],
  model: 'sonnet',
  color: 'blue',
  source: { kind: 'user' },
  file_path: '/fake/.cc-rust/agents/reviewer.md',
}

const projectEntry: AgentDefinitionEntry = {
  ...userEntry,
  system_prompt: 'Project-local reviewer.',
  source: { kind: 'project' },
  file_path: '/proj/.cc-rust/agents/reviewer.md',
}

describe('agent-settings reducer', () => {
  test('open then list populates entries and leaves dialog open', () => {
    const opened = appReducer(initialState, { type: 'AGENT_SETTINGS_OPEN' })
    const listed = appReducer(opened, {
      type: 'AGENT_SETTINGS_LIST',
      entries: [userEntry, projectEntry],
    })

    expect(listed.agentSettings.open).toBe(true)
    expect(listed.agentSettings.entries).toHaveLength(2)
    expect(listed.agentSettings.entries[0]!.name).toBe('reviewer')
  })

  test('changed with entry upserts by (name, source) — sibling sources co-exist', () => {
    const seeded = appReducer(initialState, {
      type: 'AGENT_SETTINGS_LIST',
      entries: [userEntry],
    })
    // New project-scope agent with the same name should be added, not replace.
    const afterProject = appReducer(seeded, {
      type: 'AGENT_SETTINGS_CHANGED',
      name: 'reviewer',
      entry: projectEntry,
    })

    expect(afterProject.agentSettings.entries).toHaveLength(2)
    expect(afterProject.agentSettings.entries.map(e => e.source.kind).sort()).toEqual([
      'project',
      'user',
    ])
    expect(afterProject.agentSettings.lastMessage).toBe('Saved agent: reviewer')
    expect(afterProject.agentSettings.lastError).toBeNull()
  })

  test('changed with entry replaces an existing same-source entry in place', () => {
    const seeded = appReducer(initialState, {
      type: 'AGENT_SETTINGS_LIST',
      entries: [userEntry],
    })
    const updated: AgentDefinitionEntry = { ...userEntry, description: 'Updated!' }
    const after = appReducer(seeded, {
      type: 'AGENT_SETTINGS_CHANGED',
      name: 'reviewer',
      entry: updated,
    })

    expect(after.agentSettings.entries).toHaveLength(1)
    expect(after.agentSettings.entries[0]!.description).toBe('Updated!')
  })

  test('changed with no entry removes matching-name entries', () => {
    const seeded = appReducer(initialState, {
      type: 'AGENT_SETTINGS_LIST',
      entries: [userEntry, projectEntry],
    })
    const after = appReducer(seeded, {
      type: 'AGENT_SETTINGS_CHANGED',
      name: 'reviewer',
    })

    // Fallback-path with no entry strips every entry matching the name.
    expect(after.agentSettings.entries).toHaveLength(0)
    expect(after.agentSettings.lastMessage).toBe('Deleted agent: reviewer')
  })

  test('error event surfaces on lastError and clears lastMessage', () => {
    const seeded = appReducer(initialState, { type: 'AGENT_SETTINGS_LIST', entries: [userEntry] })
    const withSuccess = appReducer(seeded, {
      type: 'AGENT_SETTINGS_CHANGED',
      name: 'reviewer',
      entry: { ...userEntry, description: 'v2' },
    })
    expect(withSuccess.agentSettings.lastMessage).not.toBeNull()

    const withError = appReducer(withSuccess, {
      type: 'AGENT_SETTINGS_ERROR',
      name: 'reviewer',
      error: 'scope is read-only',
    })
    expect(withError.agentSettings.lastError).toBe('reviewer: scope is read-only')
    expect(withError.agentSettings.lastMessage).toBeNull()
  })

  test('close resets transient notices and open flag', () => {
    const open = appReducer(initialState, { type: 'AGENT_SETTINGS_OPEN' })
    const closed = appReducer(open, { type: 'AGENT_SETTINGS_CLOSE' })
    expect(closed.agentSettings.open).toBe(false)
    expect(closed.agentSettings.lastError).toBeNull()
    expect(closed.agentSettings.lastMessage).toBeNull()
  })
})

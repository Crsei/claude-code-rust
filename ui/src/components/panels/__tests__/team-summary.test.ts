import { describe, expect, test } from 'bun:test'
import type { TeamState } from '../../../store/app-state.js'
import { summarizeTeam, summarizeTeams } from '../team-summary.js'

function member(
  agent_id: string,
  opts: {
    agent_name?: string
    role?: string
    is_active?: boolean
    unread_messages?: number
  } = {},
) {
  return {
    agent_id,
    agent_name: opts.agent_name ?? agent_id,
    role: opts.role ?? 'worker',
    is_active: opts.is_active ?? true,
    unread_messages: opts.unread_messages ?? 0,
  }
}

function team(name: string, overrides: Partial<TeamState> = {}): TeamState {
  return {
    name,
    members: overrides.members ?? [],
    pendingMessages: overrides.pendingMessages ?? 0,
    recentMessages: overrides.recentMessages ?? [],
  }
}

describe('summarizeTeam', () => {
  test('aggregates active members and unread counts', () => {
    const t = team('core', {
      members: [
        member('a', { is_active: true, unread_messages: 2 }),
        member('b', { is_active: false, unread_messages: 3 }),
        member('c', { is_active: true, unread_messages: 0 }),
      ],
      pendingMessages: 5,
    })
    const s = summarizeTeam(t)
    expect(s.name).toBe('core')
    expect(s.totalMembers).toBe(3)
    expect(s.activeMembers).toBe(2)
    expect(s.unreadTotal).toBe(5)
    expect(s.pendingMessages).toBe(5)
  })

  test('marks a team as having a lead when any member role includes "lead"', () => {
    const t = team('squad', {
      members: [
        member('a', { role: 'team-lead' }),
        member('b', { role: 'worker' }),
      ],
    })
    expect(summarizeTeam(t).hasLead).toBe(true)
  })

  test('hasLead is false when no member role mentions "lead"', () => {
    const t = team('squad', {
      members: [member('a', { role: 'worker' })],
    })
    expect(summarizeTeam(t).hasLead).toBe(false)
  })

  test('handles an empty team gracefully', () => {
    const t = team('empty')
    const s = summarizeTeam(t)
    expect(s).toEqual({
      name: 'empty',
      totalMembers: 0,
      activeMembers: 0,
      unreadTotal: 0,
      pendingMessages: 0,
      hasLead: false,
    })
  })
})

describe('summarizeTeams', () => {
  test('rolls the per-team summaries up into a single view', () => {
    const a = team('alpha', {
      members: [
        member('a1', { is_active: true, unread_messages: 2 }),
      ],
      pendingMessages: 1,
    })
    const b = team('beta', {
      members: [
        member('b1', { is_active: false, unread_messages: 4 }),
        member('b2', { is_active: true, unread_messages: 0 }),
      ],
      pendingMessages: 0,
    })
    const rollup = summarizeTeams([a, b])
    expect(rollup).toEqual({
      teamCount: 2,
      totalMembers: 3,
      activeMembers: 2,
      unreadTotal: 6,
      pendingTotal: 1,
    })
  })

  test('empty list rolls up to zeros', () => {
    expect(summarizeTeams([])).toEqual({
      teamCount: 0,
      totalMembers: 0,
      activeMembers: 0,
      unreadTotal: 0,
      pendingTotal: 0,
    })
  })
})

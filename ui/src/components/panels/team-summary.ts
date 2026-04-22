import type { TeamState } from '../../store/app-state.js'

/**
 * Pure data transforms for the team panel.
 *
 * `summarizeTeam` extracts the values that `TeamPanel.tsx` renders into
 * its header line so we can unit-test the aggregation without mounting
 * the component. `summarizeTeams` adds a cross-team roll-up used by the
 * panel's title bar.
 */

export interface TeamSummary {
  name: string
  totalMembers: number
  activeMembers: number
  unreadTotal: number
  pendingMessages: number
  hasLead: boolean
}

export interface TeamRollupSummary {
  teamCount: number
  totalMembers: number
  activeMembers: number
  unreadTotal: number
  pendingTotal: number
}

export function summarizeTeam(team: TeamState): TeamSummary {
  let unreadTotal = 0
  let activeMembers = 0
  let hasLead = false
  for (const member of team.members) {
    unreadTotal += member.unread_messages ?? 0
    if (member.is_active) activeMembers += 1
    if ((member.role ?? '').toLowerCase().includes('lead')) hasLead = true
  }
  return {
    name: team.name,
    totalMembers: team.members.length,
    activeMembers,
    unreadTotal,
    pendingMessages: team.pendingMessages,
    hasLead,
  }
}

export function summarizeTeams(teams: TeamState[]): TeamRollupSummary {
  let totalMembers = 0
  let activeMembers = 0
  let unreadTotal = 0
  let pendingTotal = 0
  for (const team of teams) {
    const s = summarizeTeam(team)
    totalMembers += s.totalMembers
    activeMembers += s.activeMembers
    unreadTotal += s.unreadTotal
    pendingTotal += s.pendingMessages
  }
  return {
    teamCount: teams.length,
    totalMembers,
    activeMembers,
    unreadTotal,
    pendingTotal,
  }
}

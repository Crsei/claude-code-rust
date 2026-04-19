import type { TeamMemberInfo } from '../../ipc/protocol.js'
import type { AppState, TeamAction } from '../app-state.js'

export function reduceTeams(state: AppState, action: TeamAction): AppState {
  switch (action.type) {
    case 'TEAM_MEMBER_JOINED': {
      const team = state.teams[action.teamName] ?? { name: action.teamName, members: [], pendingMessages: 0, recentMessages: [] }
      const newMember: TeamMemberInfo = { agent_id: action.agentId, agent_name: action.agentName, role: action.role, is_active: true, unread_messages: 0 }
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, members: [...team.members, newMember] } },
      }
    }

    case 'TEAM_MEMBER_LEFT': {
      const team = state.teams[action.teamName]
      if (!team) return state
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, members: team.members.filter(m => m.agent_id !== action.agentId) } },
      }
    }

    case 'TEAM_MESSAGE_ROUTED': {
      const team = state.teams[action.teamName] ?? { name: action.teamName, members: [], pendingMessages: 0, recentMessages: [] }
      const msg = { from: action.from, to: action.to, summary: action.summary, timestamp: action.timestamp }
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, recentMessages: [...team.recentMessages.slice(-19), msg] } },
      }
    }

    case 'TEAM_STATUS_SNAPSHOT': {
      const team = state.teams[action.teamName] ?? { name: action.teamName, members: [], pendingMessages: 0, recentMessages: [] }
      return {
        ...state,
        teams: { ...state.teams, [action.teamName]: { ...team, members: action.members, pendingMessages: action.pendingMessages } },
      }
    }
  }
}

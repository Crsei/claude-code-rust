import React from 'react'
import type { TeamMemberInfo } from '../../ipc/protocol.js'
import { c } from '../../theme.js'

/**
 * Single-row rendering for a team member. Lite-native sibling of the
 * sample tree's `TeamStatus`
 * (`ui/examples/upstream-patterns/src/components/teams/TeamStatus.tsx`)
 * — focused on the fields the Rust protocol already forwards. Role is
 * surfaced with a tint so operators can scan at a glance which rows
 * are leads vs workers.
 */

type AugmentedMember = TeamMemberInfo & { color?: string }

type Props = {
  member: AugmentedMember
}

const TEAMMATE_COLORS: Record<string, string> = {
  red: '#F38BA8',
  blue: '#89B4FA',
  green: '#A6E3A1',
  yellow: '#F9E2AF',
  purple: '#CBA6F7',
  orange: '#FAB387',
  pink: '#F5C2E7',
  cyan: '#94E2D5',
}

const DEFAULT_NAME_COLOR = '#CDD6F4'

function resolveNameColor(tag: string | undefined): string {
  if (!tag) return DEFAULT_NAME_COLOR
  return TEAMMATE_COLORS[tag.toLowerCase()] ?? DEFAULT_NAME_COLOR
}

const ROLE_COLORS: Record<string, string> = {
  lead: '#A6E3A1',
  manager: '#A6E3A1',
  worker: '#89B4FA',
  reviewer: '#CBA6F7',
}

function roleColor(role: string | undefined): string {
  if (!role) return c.dim
  const lowered = role.toLowerCase()
  for (const [key, value] of Object.entries(ROLE_COLORS)) {
    if (lowered.includes(key)) return value
  }
  return c.dim
}

export function TeamMemberCard({ member }: Props) {
  const nameColor = resolveNameColor(member.color)
  const statusIcon = member.is_active ? '●' : '○'
  const statusColor = member.is_active ? '#A6E3A1' : '#6C7086'
  const unread = member.unread_messages ?? 0
  const rolePrefix = member.role ? `[${member.role}]` : ''
  const roleFg = roleColor(member.role)

  return (
    <text>
      {'  '}
      <span fg={statusColor}>{statusIcon}</span>
      {' '}
      <span fg={nameColor}>{member.agent_name}</span>
      {rolePrefix && (
        <>
          {' '}
          <span fg={roleFg}>{rolePrefix}</span>
        </>
      )}
      {unread > 0 && (
        <span fg="#F9E2AF"> +{unread}</span>
      )}
    </text>
  )
}

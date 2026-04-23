import React from 'react'
import { c } from '../../theme.js'
import type { FeedConfig, FeedLine } from './Feed.js'

/**
 * Feed-config builders for the LogoV2 welcome screen.
 *
 * OpenTUI-native port of the upstream `LogoV2/feedConfigs`
 * (`ui/examples/upstream-patterns/src/components/LogoV2/feedConfigs.tsx`).
 * Upstream pulled data from the `LogOption` resume list, the
 * `projectOnboardingState` machine, and the referral-reward cache. The
 * Lite port accepts the data already resolved \u2014 the Rust backend owns
 * the session log / referral / onboarding subsystems.
 */

const TICK = '\u2713'

export type RecentActivity = {
  /** Rendered "5m ago" / "yesterday" timestamp. */
  timestamp: string
  /** Session / resume summary. */
  description: string
}

export function createRecentActivityFeed(
  activities: RecentActivity[],
): FeedConfig {
  const lines: FeedLine[] = activities.map(a => ({
    text: a.description,
    timestamp: a.timestamp,
  }))
  return {
    title: 'Recent activity',
    lines,
    footer: lines.length > 0 ? '/resume for more' : undefined,
    emptyMessage: 'No recent activity',
  }
}

export function createWhatsNewFeed(
  releaseNotes: string[],
  options: { antOnly?: boolean } = {},
): FeedConfig {
  const lines: FeedLine[] = releaseNotes.map(note => {
    if (options.antOnly) {
      const match = note.match(/^(\d+\s+\w+\s+ago)\s+(.+)$/)
      if (match) {
        return { timestamp: match[1], text: match[2] ?? '' }
      }
    }
    return { text: note }
  })

  const emptyMessage = options.antOnly
    ? 'Unable to fetch latest claude-cli-internal commits'
    : 'Check the Claude Code changelog for updates'

  return {
    title: options.antOnly
      ? "What's new [ANT-ONLY: Latest CC commits]"
      : "What's new",
    lines,
    footer: lines.length > 0 ? '/release-notes for more' : undefined,
    emptyMessage,
  }
}

export type OnboardingStep = {
  text: string
  isEnabled: boolean
  isComplete: boolean
}

export function createProjectOnboardingFeed(
  steps: OnboardingStep[],
  warningText?: string,
): FeedConfig {
  const enabled = steps
    .filter(s => s.isEnabled)
    .sort((a, b) => Number(a.isComplete) - Number(b.isComplete))
  const lines: FeedLine[] = enabled.map(({ text, isComplete }) => ({
    text: `${isComplete ? `${TICK} ` : ''}${text}`,
  }))
  if (warningText) {
    lines.push({ text: warningText })
  }
  return {
    title: 'Tips for getting started',
    lines,
  }
}

export function createGuestPassesFeed(
  reward?: string | null,
): FeedConfig {
  const subtitle = reward
    ? `Share Claude Code and earn ${reward} of extra usage`
    : 'Share Claude Code with friends'
  return {
    title: '3 guest passes',
    lines: [],
    customContent: {
      content: (
        <>
          <box marginTop={1} marginBottom={1}>
            <text fg={c.accent}>[\u273B] [\u273B] [\u273B]</text>
          </box>
          <text fg={c.dim}>{subtitle}</text>
        </>
      ),
      width: 48,
    },
    footer: '/passes',
  }
}

export function createOverageCreditFeed(amount?: string | null): FeedConfig {
  const FEED_SUBTITLE = 'On us. Works on third-party apps \u00B7 /extra-usage'
  const title = amount ? `${amount} in extra usage` : 'extra usage credit'
  return {
    title,
    lines: [],
    customContent: {
      content: <text fg={c.dim}>{FEED_SUBTITLE}</text>,
      width: Math.max(title.length, FEED_SUBTITLE.length),
    },
  }
}

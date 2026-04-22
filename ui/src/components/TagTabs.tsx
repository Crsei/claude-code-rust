import React from 'react'
import { c } from '../theme.js'
import { stringWidth, truncateToWidth } from './string-width.js'

/**
 * Lite-native port of the sample tree's `TagTabs`
 * (`ui/examples/upstream-patterns/src/components/TagTabs.tsx`).
 *
 * Renders a horizontally scrolling strip of tag-style tabs centered on
 * `selectedIndex`. Tabs that do not fit the available width are collapsed
 * into `← N` / `→N (tab to cycle)` hints at either edge. Pure layout:
 * keyboard handling stays with the caller.
 *
 * Width math is exported via `planTagTabs` so callers can unit-test the
 * window calculation without mounting the component.
 */

const ALL_TAB_LABEL = 'All'
const TAB_PADDING = 2
const HASH_PREFIX_LENGTH = 1
const LEFT_ARROW_PREFIX = '← '
const RIGHT_HINT_WITH_COUNT_PREFIX = '→'
const RIGHT_HINT_SUFFIX = ' (tab to cycle)'
const RIGHT_HINT_NO_COUNT = '(tab to cycle)'
const MAX_OVERFLOW_DIGITS = 2

const LEFT_ARROW_WIDTH = LEFT_ARROW_PREFIX.length + MAX_OVERFLOW_DIGITS + 1
const RIGHT_HINT_WIDTH_WITH_COUNT =
  RIGHT_HINT_WITH_COUNT_PREFIX.length +
  MAX_OVERFLOW_DIGITS +
  RIGHT_HINT_SUFFIX.length
const RIGHT_HINT_WIDTH_NO_COUNT = RIGHT_HINT_NO_COUNT.length

export type TagTabsPlan = {
  /** Clamped selected index. */
  selectedIndex: number
  /** First visible tab index, inclusive. */
  startIndex: number
  /** Last visible tab index, exclusive. */
  endIndex: number
  /** Number of tabs hidden to the left of the window. */
  hiddenLeft: number
  /** Number of tabs hidden to the right of the window. */
  hiddenRight: number
  /** Max inline width for a single tab including padding. */
  maxSingleTabWidth: number
}

type PlanInput = {
  tabs: string[]
  selectedIndex: number
  availableWidth: number
  resumeLabelWidth: number
}

function getTabWidth(tab: string, maxWidth?: number): number {
  if (tab === ALL_TAB_LABEL) {
    return ALL_TAB_LABEL.length + TAB_PADDING
  }
  const tagWidth = stringWidth(tab)
  const effectiveTagWidth = maxWidth
    ? Math.min(tagWidth, maxWidth - TAB_PADDING - HASH_PREFIX_LENGTH)
    : tagWidth
  return Math.max(0, effectiveTagWidth) + TAB_PADDING + HASH_PREFIX_LENGTH
}

function truncateTag(tag: string, maxWidth: number): string {
  const availableForTag = maxWidth - TAB_PADDING - HASH_PREFIX_LENGTH
  if (stringWidth(tag) <= availableForTag) return tag
  if (availableForTag <= 1) return tag.charAt(0)
  return truncateToWidth(tag, availableForTag)
}

export function planTagTabs({
  tabs,
  selectedIndex,
  availableWidth,
  resumeLabelWidth,
}: PlanInput): TagTabsPlan {
  const safeSelectedIndex = Math.max(0, Math.min(selectedIndex, tabs.length - 1))

  const rightHintWidth = Math.max(RIGHT_HINT_WIDTH_WITH_COUNT, RIGHT_HINT_WIDTH_NO_COUNT)
  const maxTabsWidth = availableWidth - resumeLabelWidth - rightHintWidth - 2

  const maxSingleTabWidth = Math.max(20, Math.floor(maxTabsWidth / 2))
  const tabWidths = tabs.map(tab => getTabWidth(tab, maxSingleTabWidth))

  let startIndex = 0
  let endIndex = tabs.length
  const totalTabsWidth = tabWidths.reduce(
    (sum, w, i) => sum + w + (i < tabWidths.length - 1 ? 1 : 0),
    0,
  )

  if (totalTabsWidth > maxTabsWidth && tabs.length > 0) {
    const effectiveMaxWidth = maxTabsWidth - LEFT_ARROW_WIDTH
    let windowWidth = tabWidths[safeSelectedIndex] ?? 0
    startIndex = safeSelectedIndex
    endIndex = safeSelectedIndex + 1

    while (startIndex > 0 || endIndex < tabs.length) {
      const canExpandLeft = startIndex > 0
      const canExpandRight = endIndex < tabs.length

      if (canExpandLeft) {
        const leftWidth = (tabWidths[startIndex - 1] ?? 0) + 1
        if (windowWidth + leftWidth <= effectiveMaxWidth) {
          startIndex--
          windowWidth += leftWidth
          continue
        }
      }

      if (canExpandRight) {
        const rightWidth = (tabWidths[endIndex] ?? 0) + 1
        if (windowWidth + rightWidth <= effectiveMaxWidth) {
          endIndex++
          windowWidth += rightWidth
          continue
        }
      }

      break
    }
  }

  return {
    selectedIndex: safeSelectedIndex,
    startIndex,
    endIndex,
    hiddenLeft: startIndex,
    hiddenRight: Math.max(0, tabs.length - endIndex),
    maxSingleTabWidth,
  }
}

type Props = {
  tabs: string[]
  selectedIndex: number
  availableWidth: number
  showAllProjects?: boolean
}

export function TagTabs({
  tabs,
  selectedIndex,
  availableWidth,
  showAllProjects = false,
}: Props) {
  const resumeLabel = showAllProjects ? 'Resume (All Projects)' : 'Resume'
  const resumeLabelWidth = resumeLabel.length + 1

  const plan = planTagTabs({
    tabs,
    selectedIndex,
    availableWidth,
    resumeLabelWidth,
  })
  const visibleTabs = tabs.slice(plan.startIndex, plan.endIndex)

  return (
    <box flexDirection="row" gap={1}>
      <text fg={c.accent}>{resumeLabel}</text>
      {plan.hiddenLeft > 0 && (
        <text fg={c.dim}>
          {LEFT_ARROW_PREFIX}
          {plan.hiddenLeft}
        </text>
      )}
      {visibleTabs.map((tab, i) => {
        const actualIndex = plan.startIndex + i
        const isSelected = actualIndex === plan.selectedIndex
        const displayText =
          tab === ALL_TAB_LABEL
            ? tab
            : `#${truncateTag(tab, plan.maxSingleTabWidth - TAB_PADDING)}`
        return (
          <text
            key={tab}
            fg={isSelected ? c.bg : undefined}
            bg={isSelected ? c.accent : undefined}
          >
            {' '}
            {isSelected ? <strong>{displayText}</strong> : displayText}
            {' '}
          </text>
        )
      })}
      {plan.hiddenRight > 0 ? (
        <text fg={c.dim}>
          {RIGHT_HINT_WITH_COUNT_PREFIX}
          {plan.hiddenRight}
          {RIGHT_HINT_SUFFIX}
        </text>
      ) : (
        <text fg={c.dim}>{RIGHT_HINT_NO_COUNT}</text>
      )}
    </box>
  )
}

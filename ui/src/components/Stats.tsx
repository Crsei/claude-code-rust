import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { useKeyboard } from '@opentui/react'
import { c } from '../theme.js'
import { Spinner } from './Spinner.js'

/**
 * Ported from `ui/examples/upstream-patterns/src/components/Stats.tsx`.
 *
 * Upstream mounts inside Ink, suspends on an `aggregateClaudeCodeStatsForRange`
 * promise, and relies on `asciichart` + `chalk` + `generateHeatmap` to
 * draw its charts. The Rust port surfaces the same data shape but doesn't
 * bundle those native dependencies; this file rebuilds the UI using pure
 * TypeScript with OpenTUI intrinsics and lets the caller inject the
 * aggregation + heatmap renderer through props. Callers that want the
 * original look can still wire a heatmap renderer in via `renderHeatmap`.
 *
 * Key behaviours preserved:
 *  - Two tabs: Overview / Models. `Tab` key swaps.
 *  - Date-range cycling with `r` (all \u2192 7d \u2192 30d).
 *  - `Esc` dismisses (via `onClose`).
 *  - Fun factoid below the summary cards.
 *  - Scrollable model breakdown (\u2191\u2193 when more than four models).
 */

export type ModelUsage = {
  inputTokens: number
  outputTokens: number
  cacheReadInputTokens?: number
}

export type DailyActivity = {
  date: string
  messageCount: number
}

export type DailyModelTokens = {
  date: string
  tokensByModel: Record<string, number>
}

export type ClaudeCodeStats = {
  totalSessions: number
  totalDays: number
  activeDays: number
  peakActivityDay?: string
  streaks: { currentStreak: number; longestStreak: number }
  longestSession?: { duration: number }
  modelUsage: Record<string, ModelUsage>
  dailyActivity: DailyActivity[]
  dailyModelTokens: DailyModelTokens[]
  totalSpeculationTimeSavedMs: number
}

export type StatsDateRange = '7d' | '30d' | 'all'
export type StatsTab = 'Overview' | 'Models'

const DATE_RANGE_LABELS: Record<StatsDateRange, string> = {
  '7d': 'Last 7 days',
  '30d': 'Last 30 days',
  all: 'All time',
}

const DATE_RANGE_ORDER: StatsDateRange[] = ['all', '7d', '30d']

function getNextDateRange(current: StatsDateRange): StatsDateRange {
  const i = DATE_RANGE_ORDER.indexOf(current)
  return DATE_RANGE_ORDER[(i + 1) % DATE_RANGE_ORDER.length]!
}

type Props = {
  onClose: (result?: string) => void
  /** Called when the user selects a date range — the caller is expected to
   *  return the stats for the requested range. Cached by the component. */
  loadStatsForRange: (range: StatsDateRange) => Promise<ClaudeCodeStats | null>
  /** Optional heatmap renderer — upstream uses `generateHeatmap` + Ink's
   *  `<Ansi>`; return `null` to disable the heatmap. */
  renderHeatmap?: (activity: DailyActivity[]) => React.ReactNode
  /** Called when the user presses Ctrl-S. Return the status message to
   *  show ("Copied \u2713"). */
  onCopy?: (tab: StatsTab, stats: ClaudeCodeStats) => Promise<string>
  /** Override the initial active tab (tests wire this). */
  initialTab?: StatsTab
  /** Render model name (upstream uses `renderModelName`). Defaults to identity. */
  renderModelName?: (model: string) => string
  /** Format a number with thousands separators. Defaults to `toLocaleString`. */
  formatNumber?: (n: number) => string
  /** Format a duration in milliseconds. Defaults to `Xm Xs`. */
  formatDuration?: (ms: number) => string
  /** Current terminal width — used to size the chart / heatmap. */
  columns?: number
}

const defaultRenderModelName = (model: string) => model
const defaultFormatNumber = (n: number) => n.toLocaleString('en-US')
const defaultFormatDuration = (ms: number) => {
  if (ms < 1000) return `${ms}ms`
  const totalSec = Math.floor(ms / 1000)
  if (totalSec < 60) return `${totalSec}s`
  const minutes = Math.floor(totalSec / 60)
  const seconds = totalSec % 60
  if (minutes < 60) return `${minutes}m ${seconds.toString().padStart(2, '0')}s`
  const hours = Math.floor(minutes / 60)
  const mins = minutes % 60
  return `${hours}h ${mins.toString().padStart(2, '0')}m`
}

function formatPeakDay(dateStr: string): string {
  const date = new Date(dateStr)
  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' })
}

export function Stats({
  onClose,
  loadStatsForRange,
  renderHeatmap,
  onCopy,
  initialTab = 'Overview',
  renderModelName = defaultRenderModelName,
  formatNumber = defaultFormatNumber,
  formatDuration = defaultFormatDuration,
  columns = 80,
}: Props): React.ReactElement {
  const [dateRange, setDateRange] = useState<StatsDateRange>('all')
  const [activeTab, setActiveTab] = useState<StatsTab>(initialTab)
  const [cache, setCache] =
    useState<Partial<Record<StatsDateRange, ClaudeCodeStats>>>({})
  const [isLoading, setIsLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [copyStatus, setCopyStatus] = useState<string | null>(null)

  useEffect(() => {
    if (cache[dateRange]) {
      setIsLoading(false)
      return
    }
    let cancelled = false
    setIsLoading(true)
    loadStatsForRange(dateRange)
      .then(data => {
        if (cancelled) return
        if (data === null) {
          setLoadError(null)
          setIsLoading(false)
          return
        }
        setCache(prev => ({ ...prev, [dateRange]: data }))
        setIsLoading(false)
      })
      .catch(err => {
        if (cancelled) return
        const message = err instanceof Error ? err.message : String(err)
        setLoadError(message)
        setIsLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [cache, dateRange, loadStatsForRange])

  const displayStats = cache[dateRange] ?? cache.all ?? null
  const allTimeStats = cache.all ?? displayStats

  const handleClose = useCallback(() => {
    onClose('Stats dialog dismissed')
  }, [onClose])

  useKeyboard(event => {
    if (event.eventType === 'release') return
    const name = event.name
    const seq = event.sequence

    if (name === 'escape') {
      handleClose()
      return
    }
    if (event.ctrl && (seq === 'c' || seq === 'd')) {
      handleClose()
      return
    }
    if (name === 'tab') {
      setActiveTab(prev => (prev === 'Overview' ? 'Models' : 'Overview'))
      return
    }
    if (seq === 'r' && !event.ctrl && !event.meta) {
      setDateRange(prev => getNextDateRange(prev))
      return
    }
    if (event.ctrl && seq === 's' && displayStats && onCopy) {
      void onCopy(activeTab, displayStats).then(status => {
        setCopyStatus(status)
        setTimeout(() => setCopyStatus(null), 2000)
      })
    }
  })

  if (loadError) {
    return (
      <box marginTop={1}>
        <text fg={c.error}>Failed to load stats: {loadError}</text>
      </box>
    )
  }
  if (!displayStats && isLoading) {
    return (
      <box marginTop={1} flexDirection="row">
        <Spinner label="Loading your Claude Code stats\u2026" />
      </box>
    )
  }
  if (!displayStats) {
    return (
      <box marginTop={1}>
        <text fg={c.warning}>
          No stats available yet. Start using Claude Code!
        </text>
      </box>
    )
  }

  return (
    <box
      flexDirection="column"
      borderStyle="rounded"
      borderColor={c.accent}
      paddingX={2}
      paddingY={1}
    >
      <box flexDirection="row" marginBottom={1}>
        {(['Overview', 'Models'] as StatsTab[]).map((tab, index) => (
          <box key={tab} flexDirection="row">
            {index > 0 ? <text fg={c.dim}>{'  '}</text> : null}
            <text
              fg={tab === activeTab ? c.accent : c.dim}
              bg={tab === activeTab ? c.bg : undefined}
            >
              {tab === activeTab ? <strong>{tab}</strong> : tab}
            </text>
          </box>
        ))}
      </box>

      {activeTab === 'Overview' ? (
        <OverviewTab
          stats={displayStats}
          allTimeStats={allTimeStats ?? displayStats}
          dateRange={dateRange}
          isLoading={isLoading}
          renderHeatmap={renderHeatmap}
          renderModelName={renderModelName}
          formatNumber={formatNumber}
          formatDuration={formatDuration}
        />
      ) : (
        <ModelsTab
          stats={displayStats}
          dateRange={dateRange}
          isLoading={isLoading}
          columns={columns}
          renderModelName={renderModelName}
          formatNumber={formatNumber}
        />
      )}

      <box paddingLeft={2} marginTop={1}>
        <text fg={c.dim}>
          Esc to cancel \u00b7 Tab to switch \u00b7 r to cycle dates \u00b7 Ctrl+S to copy
          {copyStatus ? ` \u00b7 ${copyStatus}` : ''}
        </text>
      </box>
    </box>
  )
}

function DateRangeSelector({
  dateRange,
  isLoading,
}: {
  dateRange: StatsDateRange
  isLoading: boolean
}): React.ReactElement {
  return (
    <box flexDirection="row" marginBottom={1}>
      {DATE_RANGE_ORDER.map((range, i) => (
        <box key={range} flexDirection="row">
          {i > 0 ? <text fg={c.dim}> \u00b7 </text> : null}
          <text fg={range === dateRange ? c.accent : c.dim}>
            {range === dateRange ? (
              <strong>{DATE_RANGE_LABELS[range]}</strong>
            ) : (
              DATE_RANGE_LABELS[range]
            )}
          </text>
        </box>
      ))}
      {isLoading ? (
        <box marginLeft={2}>
          <Spinner label="" />
        </box>
      ) : null}
    </box>
  )
}

type TabProps = {
  stats: ClaudeCodeStats
  dateRange: StatsDateRange
  isLoading: boolean
  renderModelName: (model: string) => string
  formatNumber: (n: number) => string
}

function OverviewTab({
  stats,
  allTimeStats,
  dateRange,
  isLoading,
  renderHeatmap,
  formatDuration,
  ...rest
}: TabProps & {
  allTimeStats: ClaudeCodeStats
  renderHeatmap?: (activity: DailyActivity[]) => React.ReactNode
  formatDuration: (ms: number) => string
}): React.ReactElement {
  const { renderModelName, formatNumber } = rest
  const modelEntries = Object.entries(stats.modelUsage).sort(
    ([, a], [, b]) =>
      b.inputTokens + b.outputTokens - (a.inputTokens + a.outputTokens),
  )
  const favoriteModel = modelEntries[0]
  const totalTokens = modelEntries.reduce(
    (sum, [, usage]) => sum + usage.inputTokens + usage.outputTokens,
    0,
  )

  const factoid = useMemo(
    () => generateFunFactoid(stats, totalTokens),
    [stats, totalTokens],
  )

  const rangeDays =
    dateRange === '7d' ? 7 : dateRange === '30d' ? 30 : stats.totalDays

  return (
    <box flexDirection="column" marginTop={1}>
      {allTimeStats.dailyActivity.length > 0 && renderHeatmap ? (
        <box flexDirection="column" marginBottom={1}>
          {renderHeatmap(allTimeStats.dailyActivity)}
        </box>
      ) : null}

      <DateRangeSelector dateRange={dateRange} isLoading={isLoading} />

      <box flexDirection="row" marginBottom={1}>
        <box flexDirection="column" width={32}>
          {favoriteModel ? (
            <text>
              Favorite model:{' '}
              <strong>
                <span fg={c.accent}>{renderModelName(favoriteModel[0])}</span>
              </strong>
            </text>
          ) : null}
        </box>
        <box flexDirection="column" width={32}>
          <text>
            Total tokens: <span fg={c.accent}>{formatNumber(totalTokens)}</span>
          </text>
        </box>
      </box>

      <box flexDirection="row">
        <box flexDirection="column" width={32}>
          <text>
            Sessions:{' '}
            <span fg={c.accent}>{formatNumber(stats.totalSessions)}</span>
          </text>
        </box>
        <box flexDirection="column" width={32}>
          {stats.longestSession ? (
            <text>
              Longest session:{' '}
              <span fg={c.accent}>
                {formatDuration(stats.longestSession.duration)}
              </span>
            </text>
          ) : null}
        </box>
      </box>

      <box flexDirection="row">
        <box flexDirection="column" width={32}>
          <text>
            Active days: <span fg={c.accent}>{stats.activeDays}</span>
            <span fg={c.dim}>/{rangeDays}</span>
          </text>
        </box>
        <box flexDirection="column" width={32}>
          <text>
            Longest streak:{' '}
            <strong>
              <span fg={c.accent}>{stats.streaks.longestStreak}</span>
            </strong>
            {stats.streaks.longestStreak === 1 ? ' day' : ' days'}
          </text>
        </box>
      </box>

      <box flexDirection="row">
        <box flexDirection="column" width={32}>
          {stats.peakActivityDay ? (
            <text>
              Most active day:{' '}
              <span fg={c.accent}>{formatPeakDay(stats.peakActivityDay)}</span>
            </text>
          ) : null}
        </box>
        <box flexDirection="column" width={32}>
          <text>
            Current streak:{' '}
            <strong>
              <span fg={c.accent}>{allTimeStats.streaks.currentStreak}</span>
            </strong>
            {allTimeStats.streaks.currentStreak === 1 ? ' day' : ' days'}
          </text>
        </box>
      </box>

      {stats.totalSpeculationTimeSavedMs > 0 ? (
        <box flexDirection="row">
          <box flexDirection="column" width={32}>
            <text>
              Speculation saved:{' '}
              <span fg={c.accent}>
                {formatDuration(stats.totalSpeculationTimeSavedMs)}
              </span>
            </text>
          </box>
        </box>
      ) : null}

      {factoid ? (
        <box marginTop={1}>
          <text fg={c.info}>{factoid}</text>
        </box>
      ) : null}
    </box>
  )
}

function ModelsTab({
  stats,
  dateRange,
  isLoading,
  columns,
  renderModelName,
  formatNumber,
}: TabProps & { columns: number }): React.ReactElement {
  const [scrollOffset, setScrollOffset] = useState(0)
  const VISIBLE_MODELS = 4

  const modelEntries = Object.entries(stats.modelUsage).sort(
    ([, a], [, b]) =>
      b.inputTokens + b.outputTokens - (a.inputTokens + a.outputTokens),
  )

  useKeyboard(event => {
    if (event.eventType === 'release') return
    if (event.name === 'down' && scrollOffset < modelEntries.length - VISIBLE_MODELS) {
      setScrollOffset(prev => Math.min(prev + 2, modelEntries.length - VISIBLE_MODELS))
      return
    }
    if (event.name === 'up' && scrollOffset > 0) {
      setScrollOffset(prev => Math.max(prev - 2, 0))
    }
  })

  if (modelEntries.length === 0) {
    return (
      <box>
        <text fg={c.dim}>No model usage data available</text>
      </box>
    )
  }

  const totalTokens = modelEntries.reduce(
    (sum, [, usage]) => sum + usage.inputTokens + usage.outputTokens,
    0,
  )

  const visibleModels = modelEntries.slice(
    scrollOffset,
    scrollOffset + VISIBLE_MODELS,
  )
  const midpoint = Math.ceil(visibleModels.length / 2)
  const leftModels = visibleModels.slice(0, midpoint)
  const rightModels = visibleModels.slice(midpoint)

  const canScrollUp = scrollOffset > 0
  const canScrollDown = scrollOffset < modelEntries.length - VISIBLE_MODELS
  const showScrollHint = modelEntries.length > VISIBLE_MODELS

  const colWidth = Math.max(28, Math.floor((columns - 8) / 2))

  return (
    <box flexDirection="column" marginTop={1}>
      <DateRangeSelector dateRange={dateRange} isLoading={isLoading} />

      <box flexDirection="row">
        <box flexDirection="column" width={colWidth}>
          {leftModels.map(([model, usage]) => (
            <ModelEntry
              key={model}
              model={model}
              usage={usage}
              totalTokens={totalTokens}
              renderModelName={renderModelName}
              formatNumber={formatNumber}
            />
          ))}
        </box>
        <box flexDirection="column" width={colWidth}>
          {rightModels.map(([model, usage]) => (
            <ModelEntry
              key={model}
              model={model}
              usage={usage}
              totalTokens={totalTokens}
              renderModelName={renderModelName}
              formatNumber={formatNumber}
            />
          ))}
        </box>
      </box>

      {showScrollHint ? (
        <box marginTop={1}>
          <text fg={c.dim}>
            {canScrollUp ? '\u2191' : ' '} {canScrollDown ? '\u2193' : ' '}{' '}
            {scrollOffset + 1}-
            {Math.min(scrollOffset + VISIBLE_MODELS, modelEntries.length)} of{' '}
            {modelEntries.length} models (\u2191\u2193 to scroll)
          </text>
        </box>
      ) : null}
    </box>
  )
}

type ModelEntryProps = {
  model: string
  usage: ModelUsage
  totalTokens: number
  renderModelName: (model: string) => string
  formatNumber: (n: number) => string
}

function ModelEntry({
  model,
  usage,
  totalTokens,
  renderModelName,
  formatNumber,
}: ModelEntryProps): React.ReactElement {
  const modelTokens = usage.inputTokens + usage.outputTokens
  const percentage = totalTokens > 0
    ? ((modelTokens / totalTokens) * 100).toFixed(1)
    : '0.0'

  return (
    <box flexDirection="column">
      <text>
        \u2022 <strong>{renderModelName(model)}</strong>{' '}
        <span fg={c.dim}>({percentage}%)</span>
      </text>
      <text fg={c.dim}>
        {'  '}In: {formatNumber(usage.inputTokens)} \u00b7 Out:{' '}
        {formatNumber(usage.outputTokens)}
      </text>
    </box>
  )
}

const BOOK_COMPARISONS = [
  { name: 'The Little Prince', tokens: 22000 },
  { name: 'The Old Man and the Sea', tokens: 35000 },
  { name: 'Animal Farm', tokens: 39000 },
  { name: 'The Great Gatsby', tokens: 62000 },
  { name: 'Brave New World', tokens: 83000 },
  { name: 'The Hobbit', tokens: 123000 },
  { name: '1984', tokens: 123000 },
  { name: 'Pride and Prejudice', tokens: 156000 },
  { name: 'Moby-Dick', tokens: 268000 },
  { name: 'Crime and Punishment', tokens: 274000 },
  { name: 'The Lord of the Rings', tokens: 576000 },
  { name: 'War and Peace', tokens: 730000 },
]

const TIME_COMPARISONS = [
  { name: 'a TED talk', minutes: 18 },
  { name: 'an episode of The Office', minutes: 22 },
  { name: 'a yoga class', minutes: 60 },
  { name: 'a World Cup soccer match', minutes: 90 },
  { name: 'the movie Inception', minutes: 148 },
  { name: 'a transatlantic flight', minutes: 420 },
  { name: 'a full night of sleep', minutes: 480 },
]

function generateFunFactoid(
  stats: ClaudeCodeStats,
  totalTokens: number,
): string {
  const factoids: string[] = []
  if (totalTokens > 0) {
    const matching = BOOK_COMPARISONS.filter(book => totalTokens >= book.tokens)
    for (const book of matching) {
      const times = totalTokens / book.tokens
      if (times >= 2) {
        factoids.push(
          `You've used ~${Math.floor(times)}x more tokens than ${book.name}`,
        )
      } else {
        factoids.push(`You've used the same number of tokens as ${book.name}`)
      }
    }
  }
  if (stats.longestSession) {
    const sessionMinutes = stats.longestSession.duration / (1000 * 60)
    for (const comparison of TIME_COMPARISONS) {
      const ratio = sessionMinutes / comparison.minutes
      if (ratio >= 2) {
        factoids.push(
          `Your longest session is ~${Math.floor(ratio)}x longer than ${comparison.name}`,
        )
      }
    }
  }
  if (factoids.length === 0) return ''
  return factoids[Math.floor(Math.random() * factoids.length)]!
}

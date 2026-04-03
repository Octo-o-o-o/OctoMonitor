import { memo, useEffect, useMemo, useState } from 'react'
import { useMonitorStore } from '../../store/monitorStore'
import { useI18n, type I18nKey } from '../../lib/i18n'
import { fetchCommitHistory, fetchUsageHistory } from '../../lib/history'
import {
  buildHeatmapView,
  getHeatmapScopeSelection,
  listCommitsInWindow,
  type HeatmapCell,
  type HeatmapMetric,
  type HeatmapScope,
} from '../../lib/heatmap'
import { formatDateTime, formatTokens } from '../../lib/format'
import type { CommitHistoryPayload, CommitRecord, RunRecord, UsageBucket, UsageHistoryPayload } from '../../lib/types'
import { HeatmapSkeleton } from './Skeleton'
import { buildUsageBucketIndex, intervalOverlapMs, sliceRunUsage } from '../../lib/usage'

const scopeOrder: HeatmapScope[] = ['week', 'month', 'total']
const metricOrder: HeatmapMetric[] = ['input', 'token', 'commit']
const maxSidebarCommits = 4
const metricLabelKeys: Record<HeatmapMetric, I18nKey> = {
  input: 'heatmap.metric.input',
  token: 'heatmap.metric.token',
  commit: 'heatmap.metric.commit',
}
const scopeLabelKeys: Record<HeatmapScope, I18nKey> = {
  week: 'heatmap.scope.week',
  month: 'heatmap.scope.month',
  total: 'heatmap.scope.total',
}
const totalHintKeys: Record<HeatmapMetric, I18nKey> = {
  input: 'heatmap.totalHint.input',
  token: 'heatmap.totalHint.token',
  commit: 'heatmap.totalHint.commit',
}
const surfaceHintKeys: Record<HeatmapScope, I18nKey> = {
  week: 'heatmap.surfaceHint.week',
  month: 'heatmap.surfaceHint.month',
  total: 'heatmap.surfaceHint.total',
}

type ScopeCache = {
  usage?: UsageHistoryPayload
  commits?: CommitHistoryPayload
}

function parseMs(value: string): number | undefined {
  const ms = Date.parse(value)
  return Number.isFinite(ms) ? ms : undefined
}

function filterRunsForRange(runs: RunRecord[], fromMs: number, toMs: number): RunRecord[] {
  return runs.filter((run) => {
    const startMs = parseMs(run.startedAt)
    const endMs = parseMs(run.lastActivityAt)
    if (startMs == null || endMs == null) return false
    return intervalOverlapMs(startMs, endMs, fromMs, toMs) > 0
  })
}

function filterUsageBucketsForRange(buckets: UsageBucket[], fromMs: number, toMs: number): UsageBucket[] {
  return buckets.filter((bucket) => {
    const startMs = parseMs(bucket.start)
    const endMs = parseMs(bucket.end)
    if (startMs == null || endMs == null) return false
    return intervalOverlapMs(startMs, endMs, fromMs, toMs) > 0
  })
}

function filterCommitsForRange(commits: CommitRecord[], fromMs: number, toMs: number): CommitRecord[] {
  return commits.filter((commit) => {
    const committedAtMs = parseMs(commit.committedAt)
    return committedAtMs != null && committedAtMs >= fromMs && committedAtMs <= toMs
  })
}

function padHour(hour: number): string {
  return String(hour).padStart(2, '0')
}

function weekdayShort(date: Date, locale: 'en' | 'zh'): string {
  return new Intl.DateTimeFormat(locale === 'zh' ? 'zh-CN' : 'en-US', {
    weekday: 'short',
  }).format(date)
}

function shortDate(date: Date, locale: 'en' | 'zh'): string {
  return new Intl.DateTimeFormat(locale === 'zh' ? 'zh-CN' : 'en-US', {
    month: 'numeric',
    day: 'numeric',
  }).format(date)
}

function monthLabel(date: Date, locale: 'en' | 'zh'): string {
  return new Intl.DateTimeFormat(locale === 'zh' ? 'zh-CN' : 'en-US', {
    month: 'short',
  }).format(date)
}

function exactMetricValue(value: number): string {
  return Math.round(value).toLocaleString()
}

function compactMetricValue(value: number): string {
  return formatTokens(value)
}

function selectionMetricValue(metric: HeatmapMetric, value: number): string {
  if (metric === 'token') return formatTokens(value)
  return exactMetricValue(value)
}

function selectionLabel(cell: HeatmapCell, locale: 'en' | 'zh'): string {
  const dayLabel = `${shortDate(cell.date, locale)} ${weekdayShort(cell.date, locale)}`
  if (cell.hour == null) return dayLabel
  return `${dayLabel} ${padHour(cell.hour)}:00 - ${padHour(cell.hour + 1)}:00`
}

function commitMetaLabel(commit: CommitRecord): string {
  return `${commit.repoName} · ${commit.shortSha} · ${formatDateTime(commit.committedAt).slice(5, 16)}`
}

export const HeatmapView = memo(function HeatmapView() {
  const data = useMonitorStore((s) => s.data)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const { locale, t } = useI18n()
  const [scope, setScope] = useState<HeatmapScope>('week')
  const [metric, setMetric] = useState<HeatmapMetric>('token')
  const [hoveredCellKey, setHoveredCellKey] = useState<string>()
  const [pinnedCellKey, setPinnedCellKey] = useState<string>()
  const [historyCache, setHistoryCache] = useState<Partial<Record<HeatmapScope, ScopeCache>>>({})
  const [loadingScope, setLoadingScope] = useState<HeatmapScope | null>(null)
  const [errorScopes, setErrorScopes] = useState<Partial<Record<HeatmapScope, boolean>>>({})

  const range = useMemo(() => getHeatmapScopeSelection(scope), [scope])
  const fromMs = range.from.getTime()
  const toMs = range.to.getTime()
  const cachedScope = historyCache[scope]

  useEffect(() => {
    if (cachedScope?.usage && cachedScope?.commits) return

    const controller = new AbortController()
    setLoadingScope(scope)
    setErrorScopes((prev) => ({ ...prev, [scope]: false }))

    const usagePromise = cachedScope?.usage
      ? Promise.resolve(cachedScope.usage)
      : fetchUsageHistory(range, controller.signal)
    const commitPromise = cachedScope?.commits
      ? Promise.resolve(cachedScope.commits)
      : fetchCommitHistory(range, controller.signal)

    void Promise.allSettled([usagePromise, commitPromise])
      .then(([usageResult, commitResult]) => {
        if (controller.signal.aborted) return

        setHistoryCache((prev) => {
          const nextScope: ScopeCache = { ...(prev[scope] ?? {}) }
          if (usageResult.status === 'fulfilled') nextScope.usage = usageResult.value
          if (commitResult.status === 'fulfilled') nextScope.commits = commitResult.value
          return { ...prev, [scope]: nextScope }
        })

        if (usageResult.status === 'rejected' || commitResult.status === 'rejected') {
          setErrorScopes((prev) => ({ ...prev, [scope]: true }))
        }
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setLoadingScope((current) => (current === scope ? null : current))
        }
      })

    return () => controller.abort()
  }, [cachedScope?.commits, cachedScope?.usage, range, scope])

  const snapshotRuns = useMemo(
    () => filterRunsForRange(data?.runs ?? [], fromMs, toMs),
    [data?.runs, fromMs, toMs],
  )
  const snapshotBuckets = useMemo(
    () => filterUsageBucketsForRange(data?.usageBuckets ?? [], fromMs, toMs),
    [data?.usageBuckets, fromMs, toMs],
  )
  const snapshotCommits = useMemo(
    () => filterCommitsForRange(data?.commits ?? [], fromMs, toMs),
    [data?.commits, fromMs, toMs],
  )

  const activeRuns = cachedScope?.usage?.runs ?? snapshotRuns
  const activeUsageBuckets = cachedScope?.usage?.usageBuckets ?? snapshotBuckets
  const activeCommits = cachedScope?.commits?.commits ?? snapshotCommits
  const truncated = Boolean(cachedScope?.usage?.truncated || cachedScope?.commits?.truncated)
  const loading = loadingScope === scope
  const hasHistoryData = Boolean(cachedScope?.usage || cachedScope?.commits)
  const error = Boolean(errorScopes[scope])

  const heatmap = useMemo(
    () => buildHeatmapView({
      scope,
      metric,
      now: range.to,
      runs: activeRuns,
      usageBuckets: activeUsageBuckets,
      commits: activeCommits,
    }),
    [activeCommits, activeRuns, activeUsageBuckets, metric, range.to, scope],
  )

  useEffect(() => {
    if (pinnedCellKey && !heatmap.cells.some((cell) => !cell.hidden && cell.key === pinnedCellKey)) {
      setPinnedCellKey(undefined)
    }
    if (hoveredCellKey && !heatmap.cells.some((cell) => !cell.hidden && cell.key === hoveredCellKey)) {
      setHoveredCellKey(undefined)
    }
  }, [heatmap, hoveredCellKey, pinnedCellKey])

  function handleCellHover(cellKey: string) {
    if (pinnedCellKey) return
    setHoveredCellKey(cellKey)
  }

  function handleCellClick(cellKey: string) {
    setHoveredCellKey(cellKey)
    setPinnedCellKey((current) => (current === cellKey ? undefined : cellKey))
  }

  const selectedCellKey = pinnedCellKey ?? hoveredCellKey

  const selectedCell = useMemo(
    () => heatmap.cells.find((cell) => !cell.hidden && cell.key === selectedCellKey) ?? heatmap.summary.peakCell,
    [heatmap, selectedCellKey],
  )
  const selectedCommits = useMemo(
    () => listCommitsInWindow(activeCommits, selectedCell.startMs, selectedCell.endMs),
    [activeCommits, selectedCell],
  )
  const selectedUsage = useMemo(() => {
    const usageBucketIndex = buildUsageBucketIndex(activeUsageBuckets)
    let input = 0

    for (const run of activeRuns) {
      const slice = sliceRunUsage(run, usageBucketIndex.get(run.id), selectedCell.startMs, selectedCell.endMs)
      input += slice.messageCount
    }

    return { input }
  }, [activeRuns, activeUsageBuckets, selectedCell.endMs, selectedCell.startMs])
  const visibleSidebarCommits = selectedCommits.slice(0, maxSidebarCommits)
  const extraCommitCount = Math.max(selectedCommits.length - visibleSidebarCommits.length, 0)

  if (!data && loading && !hasHistoryData) {
    return <HeatmapSkeleton />
  }

  const dynamicSummaryLabel = scope === 'total' ? t('heatmap.longestStreak') : t('heatmap.activeDays')
  const dynamicSummaryValue = scope === 'total'
    ? `${heatmap.summary.longestStreak}d`
    : String(heatmap.summary.activeDays)
  const dynamicSummaryHint = scope === 'total'
    ? t('heatmap.longestStreakHint')
    : t('heatmap.activeDaysHint')

  return (
    <div className="heatmap-view" data-metric={metric}>
      {connectionStatus === 'offline' && (
        <div className="status-notice offline">
          <strong>{t('heatmap.offlineTitle')}</strong>
          <span>{t('heatmap.offlineHint')}</span>
        </div>
      )}
      {loading && (
        <div className="status-notice">
          <strong>{t('heatmap.loading')}</strong>
          <span>{t('heatmap.loadingHint')}</span>
        </div>
      )}
      {error && (
        <div className="status-notice offline">
          <strong>{t('history.error')}</strong>
          <span>{t('heatmap.errorHint')}</span>
        </div>
      )}
      {truncated && (
        <div className="status-notice warn">
          <strong>{t('history.truncated')}</strong>
        </div>
      )}

      <div className="heatmap-toolbar">
        <div className="heatmap-toolbar-left">
          <div className="heatmap-inline-control">
            <span className="heatmap-inline-label">{t('heatmap.metric')}</span>
            <div className="data-mode-switch heatmap-switch heatmap-metric-switch">
              {metricOrder.map((option) => (
                <button
                  key={option}
                  className={`data-mode-btn ${metric === option ? 'active' : ''}`}
                  onClick={() => setMetric(option)}
                  type="button"
                >
                  {t(metricLabelKeys[option])}
                </button>
              ))}
            </div>
          </div>

          <span className="heatmap-toolbar-sep" aria-hidden="true" />

          <div className="heatmap-inline-control">
            <span className="heatmap-inline-label">{t('heatmap.scope')}</span>
            <div className="data-mode-switch heatmap-switch heatmap-scope-switch">
              {scopeOrder.map((option) => (
                <button
                  key={option}
                  className={`data-mode-btn ${scope === option ? 'active' : ''}`}
                  onClick={() => setScope(option)}
                  type="button"
                >
                  {t(scopeLabelKeys[option])}
                </button>
              ))}
            </div>
          </div>
        </div>
      </div>

      <div className="heatmap-summary-grid">
        <div className="summary-stat heatmap-summary-card">
          <span className="summary-label">{t('heatmap.total')}</span>
          <strong className="summary-value">{compactMetricValue(heatmap.summary.total)}</strong>
          <span className="heatmap-summary-hint">{t(totalHintKeys[metric])}</span>
        </div>
        <div className="summary-stat heatmap-summary-card">
          <span className="summary-label">{t('heatmap.peakCell')}</span>
          <strong className="summary-value">{compactMetricValue(heatmap.summary.peakCell.value)}</strong>
          <span className="heatmap-summary-hint">
            {heatmap.summary.peakCell.hour == null ? t('heatmap.peakCellHint.day') : t('heatmap.peakCellHint.hour')}
          </span>
        </div>
        <div className="summary-stat heatmap-summary-card">
          <span className="summary-label">{t('heatmap.topDay')}</span>
          <strong className="summary-value">{compactMetricValue(heatmap.summary.topDay.total)}</strong>
          <span className="heatmap-summary-hint">
            {`${shortDate(heatmap.summary.topDay.date, locale)} ${weekdayShort(heatmap.summary.topDay.date, locale)}`}
          </span>
        </div>
        <div className="summary-stat heatmap-summary-card">
          <span className="summary-label">{dynamicSummaryLabel}</span>
          <strong className="summary-value">{dynamicSummaryValue}</strong>
          <span className="heatmap-summary-hint">{dynamicSummaryHint}</span>
        </div>
      </div>

      <div className="heatmap-layout">
        <section className="page-section heatmap-panel">
          <div className="heatmap-panel-head">
            <div className="heatmap-panel-copy">
              <span className="usage-section-label">{t('heatmap.surfaceTitle')}</span>
              <p className="heatmap-panel-hint">{t(surfaceHintKeys[scope])}</p>
            </div>
            <div className="heatmap-legend">
              <span>{t('heatmap.legend.less')}</span>
              <div className="heatmap-legend-swatches">
                <span className="heatmap-legend-cell" data-level="0" />
                <span className="heatmap-legend-cell" data-level="1" />
                <span className="heatmap-legend-cell" data-level="2" />
                <span className="heatmap-legend-cell" data-level="3" />
                <span className="heatmap-legend-cell" data-level="4" />
              </div>
              <span>{t('heatmap.legend.more')}</span>
            </div>
          </div>

          {heatmap.kind === 'hourly' ? (
            <div className={`heatmap-hour-grid scope-${scope}`}>
              <div className="heatmap-hour-axis">
                <div className="heatmap-axis-spacer" />
                {Array.from({ length: 24 }, (_, hour) => (
                  <span key={hour} className="heatmap-hour-axis-label">{padHour(hour)}</span>
                ))}
              </div>
              {heatmap.rows.map((row) => (
                <div key={row.key} className="heatmap-hour-row">
                  <div className="heatmap-day-label">
                    <strong>{shortDate(row.date, locale)}</strong>
                    <span>{weekdayShort(row.date, locale)}</span>
                  </div>
                  <div className="heatmap-hour-cells">
                    {row.cells.map((cell) => (
                      <button
                        key={cell.key}
                        type="button"
                        className={`heatmap-cell ${selectedCell.key === cell.key ? 'selected' : ''}`}
                        data-level={cell.level}
                        aria-label={`${selectionLabel(cell, locale)} ${exactMetricValue(cell.value)}`}
                        onMouseEnter={() => handleCellHover(cell.key)}
                        onFocus={() => handleCellHover(cell.key)}
                        onClick={() => handleCellClick(cell.key)}
                      />
                    ))}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="heatmap-calendar">
              <div className="heatmap-calendar-months">
                {heatmap.monthMarkers.map((marker) => (
                  <span
                    key={marker.key}
                    className="heatmap-calendar-month"
                    style={{ left: `calc(52px + ${marker.column} * (var(--calendar-cell) + var(--calendar-gap)))` }}
                  >
                    {monthLabel(marker.date, locale)}
                  </span>
                ))}
              </div>
              <div className="heatmap-calendar-body">
                <div className="heatmap-calendar-weekdays">
                  {Array.from({ length: 7 }, (_, index) => {
                    const anchor = new Date(2026, 0, 5 + index)
                    return (
                      <span key={index} className="heatmap-calendar-weekday">
                        {weekdayShort(anchor, locale)}
                      </span>
                    )
                  })}
                </div>
                <div className="heatmap-calendar-columns">
                  {heatmap.columns.map((column) => (
                    <div key={column.key} className="heatmap-calendar-column">
                      {column.cells.map((cell) => (
                        cell.hidden
                          ? <span key={cell.key} className="heatmap-cell heatmap-cell-hidden" />
                          : (
                            <button
                              key={cell.key}
                              type="button"
                              className={`heatmap-cell ${selectedCell.key === cell.key ? 'selected' : ''}`}
                              data-level={cell.level}
                              aria-label={`${selectionLabel(cell, locale)} ${exactMetricValue(cell.value)}`}
                              onMouseEnter={() => handleCellHover(cell.key)}
                              onFocus={() => handleCellHover(cell.key)}
                              onClick={() => handleCellClick(cell.key)}
                            />
                          )
                      ))}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          )}
        </section>

        <aside className="page-section heatmap-side-panel">
          <div className="heatmap-selection-card">
            <div className="heatmap-selection-head">
              <span className="usage-section-label">{t('heatmap.selection')}</span>
              <span className={`heatmap-selection-state${pinnedCellKey ? ' pinned' : ''}`}>
                {pinnedCellKey ? t('heatmap.selectionPinned') : t('heatmap.selectionPreview')}
              </span>
            </div>
            <strong className="heatmap-selection-value">{selectionMetricValue(metric, selectedCell.value)}</strong>
            {metric !== 'input' && (
              <div className="heatmap-selection-supporting">
                <span className="heatmap-selection-supporting-label">{t(metricLabelKeys.input)}</span>
                <strong className="heatmap-selection-supporting-value">{exactMetricValue(selectedUsage.input)}</strong>
              </div>
            )}
            <div className="heatmap-selection-meta">{selectionLabel(selectedCell, locale)}</div>
            <div className="heatmap-selection-hint">
              {selectedCell.hour == null ? t('heatmap.selectedDayHint') : t('heatmap.selectedHourHint')}
            </div>
          </div>

          <div className="heatmap-commit-card">
            <div className="heatmap-commit-head">
              <span className="usage-section-label">{t('heatmap.commitSummaries')}</span>
              <span className="heatmap-commit-count">{selectedCommits.length}</span>
            </div>
            {visibleSidebarCommits.length === 0 ? (
              <div className="empty-state-panel heatmap-empty">
                <strong>{t('heatmap.noCommits')}</strong>
                <span>{t('heatmap.noCommitsHint')}</span>
              </div>
            ) : (
              <div className="heatmap-commit-list">
                {visibleSidebarCommits.map((commit) => (
                  <article key={commit.id} className="heatmap-commit-item">
                    <div className="heatmap-commit-meta">{commitMetaLabel(commit)}</div>
                    <div className="heatmap-commit-summary">{commit.summary}</div>
                  </article>
                ))}
                {extraCommitCount > 0 && (
                  <div className="heatmap-more-commits">
                    {t('heatmap.moreCommits').replace('{count}', String(extraCommitCount))}
                  </div>
                )}
              </div>
            )}
          </div>
        </aside>
      </div>
    </div>
  )
})

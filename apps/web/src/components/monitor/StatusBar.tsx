import { useMonitorStore, type ActiveTab, type MonitorPeriod } from '../../store/monitorStore'
import { useI18n } from '../../lib/i18n'
import { buildUsageDateRange } from '../../lib/dateRange'
import { formatTokens } from '../../lib/format'
import { buildMonitorStateStats, periodToMs } from '../../lib/monitor'
import { buildSnapshotRange } from '../../lib/snapshotWindow'
import {
  buildUsageBucketIndex,
  collectRunUsageSlicesFromIndex,
  runOverlapMs,
  runOverlapsRange,
  sumUsageSlices,
} from '../../lib/usage'
import { useTheme, builtinThemeOrder, builtinThemeIcons, builtinThemeLabels } from '../../lib/theme'
import { useEffect, useMemo, useRef, useState, type MouseEvent } from 'react'
import type { RunRecord } from '../../lib/types'
import type { RuntimeMode } from '../../lib/runtimeMode'
import { SnapshotWindowSwitch } from './SnapshotWindowSwitch'

const periodLabels: Record<MonitorPeriod, string> = {
  '30m': '30M',
  '1h': '1H',
  '2h': '2H',
  '4h': '4H',
  '8h': '8H',
  '24h': '24H',
}

function computeActiveSeconds(runs: RunRecord[], cutoff: number, now: number): number {
  const intervals: [number, number][] = []
  for (const r of runs) {
    const rawStart = new Date(r.startedAt).getTime()
    if (!Number.isFinite(rawStart)) continue
    const overlap = runOverlapMs(r, cutoff, now)
    if (overlap <= 0) continue
    const start = Math.max(rawStart, cutoff)
    const end = start + overlap
    if (end > start) intervals.push([start, end])
  }
  if (intervals.length === 0) return 0
  intervals.sort((a, b) => a[0] - b[0])
  let totalMs = 0
  let [curStart, curEnd] = intervals[0]
  for (let i = 1; i < intervals.length; i++) {
    const [s, e] = intervals[i]
    if (s <= curEnd) {
      curEnd = Math.max(curEnd, e)
    } else {
      totalMs += curEnd - curStart
      curStart = s
      curEnd = e
    }
  }
  totalMs += curEnd - curStart
  return totalMs / 1000
}

function useMeasuredWidth<T extends HTMLElement>() {
  const ref = useRef<T | null>(null)
  const [width, setWidth] = useState(Number.POSITIVE_INFINITY)

  useEffect(() => {
    const node = ref.current
    if (!node) return

    const update = () => {
      const nextWidth = Math.round(node.getBoundingClientRect().width || node.clientWidth)
      if (nextWidth > 0) setWidth(nextWidth)
    }

    update()

    if (typeof ResizeObserver !== 'undefined') {
      const observer = new ResizeObserver(() => update())
      observer.observe(node)
      return () => observer.disconnect()
    }

    window.addEventListener('resize', update)
    return () => window.removeEventListener('resize', update)
  }, [])

  return { ref, width }
}

export function StatusBar({ runtimeMode, wsConnected }: { runtimeMode: RuntimeMode; wsConnected: boolean }) {
  const data = useMonitorStore((s) => s.data)
  const activeTab = useMonitorStore((s) => s.activeTab)
  const setActiveTab = useMonitorStore((s) => s.setActiveTab)
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)
  const snapshotWindow = useMonitorStore((s) => s.settings.snapshotWindow)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const { locale, setLocale, t } = useI18n()
  const { themeId, setTheme } = useTheme()
  const { ref: headerRef, width: statusBarWidth } = useMeasuredWidth<HTMLElement>()
  const tabs: ActiveTab[] = runtimeMode === 'remoteViewer'
    ? ['monitor', 'usage']
    : ['monitor', 'usage', 'commits', 'heatmap', 'settings']
  const usageBucketIndex = useMemo(
    () => buildUsageBucketIndex(data?.usageBuckets ?? []),
    [data?.usageBuckets],
  )
  const snapshotUsageRange = useMemo(
    () => (data ? buildSnapshotRange(snapshotWindow, buildUsageDateRange(data.runs, data.usageBuckets)) : null),
    [data, snapshotWindow],
  )

  const stats = useMemo(() => {
    if (!data) return { active: 0, waiting: 0, done: 0, tokens: 0, usd: 0, tps: 0 }
    const { active, waiting, done } = buildMonitorStateStats(data.runs, monitorPeriod)

    const now = Date.now()
    const cutoff = now - (periodToMs[monitorPeriod] ?? periodToMs['1h'])

    const monitorUsageSlices = collectRunUsageSlicesFromIndex(data.runs, usageBucketIndex, cutoff, now)
    const runsInPeriod = monitorUsageSlices
      .filter(({ run }) => runOverlapsRange(run, cutoff, now))
      .map(({ run }) => run)
    const usageFrom = snapshotUsageRange?.from.getTime() ?? cutoff
    const usageTo = snapshotUsageRange?.to.getTime() ?? now
    const snapshotUsageSlices = collectRunUsageSlicesFromIndex(data.runs, usageBucketIndex, usageFrom, usageTo)
    const usageTotals = sumUsageSlices(snapshotUsageSlices.map(({ usage }) => usage))
    const tokens = usageTotals.totalTokens
    const usd = usageTotals.costUsd ?? 0

    const monitorUsageTotals = sumUsageSlices(monitorUsageSlices.map(({ usage }) => usage))
    const ioTokens = monitorUsageTotals.inputTokens + monitorUsageTotals.outputTokens
    const activeSec = computeActiveSeconds(runsInPeriod, cutoff, now)
    const tps = activeSec > 0 ? ioTokens / activeSec : 0

    return { active, waiting, done, tokens, usd, tps }
  }, [data, monitorPeriod, snapshotUsageRange, usageBucketIndex])

  function cycleTheme() {
    const idx = builtinThemeOrder.indexOf(themeId)
    const next = builtinThemeOrder[(idx + 1) % builtinThemeOrder.length]
    setTheme(next)
  }

  const themeIcon = builtinThemeIcons[themeId] ?? builtinThemeIcons.dark
  const themeLabel = builtinThemeLabels[themeId] ?? themeId
  const activeTabLabel = t(`tab.${activeTab}`)
  const showToolbarActions = statusBarWidth > 1480
  const showSecondaryStats = statusBarWidth > 1330
  const showPrimaryStats = statusBarWidth > 1180
  const useTabsMenu = statusBarWidth <= 1040
  const showUsageWindowPill = statusBarWidth > 920
  const showStatusStats = statusBarWidth > 840

  function handleMenuTabSelect(event: MouseEvent<HTMLButtonElement>, tab: ActiveTab) {
    setActiveTab(tab)
    const details = event.currentTarget.closest('details')
    if (details instanceof HTMLDetailsElement) {
      details.open = false
    }
  }

  return (
    <header ref={headerRef} className="status-bar">
      <div className="status-bar-left">
        <span className={`live-indicator ${wsConnected ? 'connected' : 'disconnected'}`}>
          <span className="live-dot" />
          {wsConnected ? t('ws.live') : t('ws.offline')}
        </span>
        {useTabsMenu ? (
          <details className="status-tabs-menu">
            <summary className="status-tabs-menu-button">
              <span>{activeTabLabel}</span>
              <span className="status-tabs-menu-caret" aria-hidden="true">v</span>
            </summary>
            <div className="status-tabs-menu-popover">
              {tabs.map((tab) => (
                <button
                  key={tab}
                  className={`status-tabs-menu-item ${activeTab === tab ? 'active' : ''}`}
                  onClick={(event) => handleMenuTabSelect(event, tab)}
                  type="button"
                >
                  {t(`tab.${tab}`)}
                </button>
              ))}
            </div>
          </details>
        ) : (
          <nav className="view-tabs" role="tablist">
            {tabs.map((tab) => (
              <button
                key={tab}
                className={`view-tab ${activeTab === tab ? 'active' : ''}`}
                onClick={() => setActiveTab(tab)}
                role="tab"
                aria-selected={activeTab === tab}
                type="button"
              >
                {t(`tab.${tab}`)}
              </button>
            ))}
          </nav>
        )}
      </div>
      <div className="status-bar-right">
        {showStatusStats && (
          <div className="status-stats">
            {showPrimaryStats && (
              <div className="status-stat-group status-stat-group-primary">
                <span className="stat">
                  {t('stat.active')} <strong className="stat-active">{stats.active}</strong>
                </span>
                {stats.waiting > 0 && (
                  <span className="stat">
                    {t('stat.wait')} <strong className="stat-wait">{stats.waiting}</strong>
                  </span>
                )}
                <span className="stat">
                  {t('stat.done')} <strong className="stat-done">{stats.done}</strong>
                </span>
              </div>
            )}
            {showSecondaryStats && (
              <div className="status-stat-group status-stat-group-secondary">
                <span className="stat">
                  {t('stat.tps')} <strong>{stats.tps.toFixed(1)}</strong>
                </span>
                <span className="stat">
                  {t('stat.tok')} <strong>{formatTokens(stats.tokens)}</strong>
                </span>
                <span className="stat">
                  {t('stat.usd')} <strong>${stats.usd.toFixed(2)}</strong>
                </span>
              </div>
            )}
            <div className="status-stat-group status-stat-group-tertiary">
              {showUsageWindowPill && <span className="usage-window-pill">{periodLabels[monitorPeriod]}</span>}
              <SnapshotWindowSwitch
                compact
                value={snapshotWindow}
                onChange={(nextWindow) => updateSettings({ snapshotWindow: nextWindow })}
              />
            </div>
          </div>
        )}
        {showToolbarActions && (
          <div className="toolbar-actions">
            <button
              className="toolbar-btn"
              onClick={() => setLocale(locale === 'en' ? 'zh' : 'en')}
              title={t('settings.language')}
              type="button"
            >
              {locale === 'en' ? 'EN' : '\u4E2D'}
            </button>
            <button
              className="toolbar-btn"
              onClick={cycleTheme}
              title={`${t('settings.theme')}: ${themeLabel}`}
              type="button"
            >
              {themeIcon}
            </button>
          </div>
        )}
      </div>
    </header>
  )
}

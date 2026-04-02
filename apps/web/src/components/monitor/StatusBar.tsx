import { useMonitorStore, type ActiveTab, type MonitorPeriod, type UsageWindow } from '../../store/monitorStore'
import { useI18n, type Locale } from '../../lib/i18n'
import { formatTokens } from '../../lib/format'
import { buildMonitorStateStats, periodToMs } from '../../lib/monitor'
import { collectRunUsageSlices, runOverlapMs, runOverlapsRange, sumUsageSlices } from '../../lib/usage'
import { useTheme, builtinThemeOrder, builtinThemeIcons, builtinThemeLabels } from '../../lib/theme'
import { useMemo } from 'react'
import type { RunRecord } from '../../lib/types'
import type { RuntimeMode } from '../../lib/runtimeMode'

const usageWindows: UsageWindow[] = ['live', 'day', 'week', 'month', 'all']

const periodLabels: Record<MonitorPeriod, string> = {
  '30m': '30M',
  '1h': '1H',
  '2h': '2H',
  '4h': '4H',
  '8h': '8H',
  '24h': '24H',
}

function getWindowCutoff(window: Exclude<UsageWindow, 'live'>): Date {
  const now = new Date()
  switch (window) {
    case 'day': return new Date(now.getTime() - 24 * 60 * 60 * 1000)
    case 'week': return new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000)
    case 'month': return new Date(now.getTime() - 30 * 24 * 60 * 60 * 1000)
    case 'all': return new Date(0)
  }
}

/** Compute the union of active time intervals and return total seconds */
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

export function StatusBar({ runtimeMode, wsConnected }: { runtimeMode: RuntimeMode; wsConnected: boolean }) {
  const data = useMonitorStore((s) => s.data)
  const activeTab = useMonitorStore((s) => s.activeTab)
  const setActiveTab = useMonitorStore((s) => s.setActiveTab)
  const usageWindow = useMonitorStore((s) => s.settings.usageWindow)
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const { locale, setLocale, t } = useI18n()
  const { themeId, setTheme } = useTheme()
  const tabs: ActiveTab[] = runtimeMode === 'remoteViewer'
    ? ['monitor', 'usage', 'commits']
    : ['monitor', 'usage', 'commits', 'settings']

  const stats = useMemo(() => {
    if (!data) return { active: 0, waiting: 0, done: 0, tokens: 0, usd: 0, tps: 0 }
    const { active, waiting, done } = buildMonitorStateStats(data.runs, monitorPeriod)

    // Determine the effective cutoff for token/cost stats
    const now = Date.now()
    let cutoffMs: number
    if (usageWindow === 'live') {
      // "live" now uses the monitorPeriod setting
      cutoffMs = periodToMs[monitorPeriod]
    } else if (usageWindow === 'all') {
      cutoffMs = now
    } else {
      const cutoffDate = getWindowCutoff(usageWindow)
      cutoffMs = now - cutoffDate.getTime()
    }
    const cutoff = now - cutoffMs

    const runUsageSlices = collectRunUsageSlices(data.runs, data.usageBuckets, cutoff, now)
    const usageTotals = sumUsageSlices(runUsageSlices.map(({ usage }) => usage))
    const runsInPeriod = runUsageSlices
      .filter(({ run }) => runOverlapsRange(run, cutoff, now))
      .map(({ run }) => run)
    const tokens = usageTotals.totalTokens
    const usd = usageTotals.costUsd ?? 0

    // TPS = (input + output) tokens / active seconds
    // Excludes cacheRead/cacheWrite — cache hits are not real compute work
    const ioTokens = usageTotals.inputTokens + usageTotals.outputTokens
    const activeSec = computeActiveSeconds(runsInPeriod, cutoff, now)
    const tps = activeSec > 0 ? ioTokens / activeSec : 0

    return { active, waiting, done, tokens, usd, tps }
  }, [data, usageWindow, monitorPeriod])

  function cycleTheme() {
    const idx = builtinThemeOrder.indexOf(themeId)
    const next = builtinThemeOrder[(idx + 1) % builtinThemeOrder.length]
    setTheme(next)
  }

  const themeIcon = builtinThemeIcons[themeId] ?? builtinThemeIcons.dark
  const themeLabel = builtinThemeLabels[themeId] ?? themeId

  return (
    <header className="status-bar">
      <div className="status-bar-left">
        <span className={`live-indicator ${wsConnected ? 'connected' : 'disconnected'}`}>
          <span className="live-dot" />
          {wsConnected ? t('ws.live') : t('ws.offline')}
        </span>
        <nav className="view-tabs" role="tablist">
          {tabs.map((tab) => (
            <button
              key={tab}
              className={`view-tab ${activeTab === tab ? 'active' : ''}`}
              onClick={() => setActiveTab(tab)}
              role="tab"
              aria-selected={activeTab === tab}
            >
              {t(`tab.${tab}`)}
            </button>
          ))}
        </nav>
      </div>
      <div className="status-bar-right">
        <div className="status-stats">
          <span className="stat">
            {t('stat.active')} <strong className="stat-active">{stats.active}</strong>
          </span>
          <span className="stat">
            {t('stat.wait')} <strong className="stat-wait">{stats.waiting}</strong>
          </span>
          <span className="stat">
            {t('stat.done')} <strong className="stat-done">{stats.done}</strong>
          </span>
          <span className="stat-sep" />
          <span className="stat">
            {t('stat.tps')} <strong>{stats.tps.toFixed(1)}</strong>
          </span>
          <span className="stat">
            {t('stat.tok')} <strong>{formatTokens(stats.tokens)}</strong>
          </span>
          <span className="stat">
            {t('stat.usd')} <strong>${stats.usd.toFixed(2)}</strong>
          </span>
          <span className="stat-sep" />
          <span className="usage-window-toggle">
            {usageWindows.map((w) => (
              <button
                key={w}
                className={`usage-window-btn ${usageWindow === w ? 'active' : ''}`}
                onClick={() => updateSettings({ usageWindow: w })}
              >
                {w === 'live' ? periodLabels[monitorPeriod] : t(`stat.${w}`)}
              </button>
            ))}
          </span>
        </div>
        <div className="toolbar-actions">
          <button
            className="toolbar-btn"
            onClick={() => setLocale(locale === 'en' ? 'zh' : 'en' as Locale)}
            title={t('settings.language')}
          >
            {locale === 'en' ? 'EN' : '\u4E2D'}
          </button>
          <button
            className="toolbar-btn"
            onClick={cycleTheme}
            title={`${t('settings.theme')}: ${themeLabel}`}
          >
            {themeIcon}
          </button>
        </div>
      </div>
    </header>
  )
}

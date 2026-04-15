import { useCallback, useEffect, useRef, useState } from 'react'
import {
  useMonitorStore,
  type DailySummaryMode,
  type HistoricalReportEntry,
} from '../../../store/monitorStore'
import { useI18n } from '../../../lib/i18n'
import { apiFetch } from '../../../lib/api'
import { pad2 } from '../../../lib/format'
import { buildAiPrompt, buildBasicReport, buildDailySummary } from '../../../lib/dailySummary'
import { fetchCommitHistory, fetchUsageHistory } from '../../../lib/history'

const modes: DailySummaryMode[] = ['claude', 'codex', 'basic']
const dayStartOptions = [0, 2, 4, 6, 8, 10]

interface ToolCapability {
  tool: string
  detected: boolean
}

function datesInRange(from: string, to: string): string[] {
  const dates: string[] = []
  const start = new Date(from + 'T00:00:00')
  const end = new Date(to + 'T00:00:00')
  if (isNaN(start.getTime()) || isNaN(end.getTime()) || start > end) return dates
  const cursor = new Date(start)
  while (cursor <= end) {
    dates.push(cursor.toISOString().slice(0, 10))
    cursor.setDate(cursor.getDate() + 1)
  }
  return dates
}

function todayStr(): string {
  return new Date().toISOString().slice(0, 10)
}

function yesterdayStr(): string {
  const d = new Date()
  d.setDate(d.getDate() - 1)
  return d.toISOString().slice(0, 10)
}

function weekAgoStr(): string {
  const d = new Date()
  d.setDate(d.getDate() - 7)
  return d.toISOString().slice(0, 10)
}

async function generateOneDay(
  dateStr: string,
  dayStartHour: number,
  mode: DailySummaryMode,
  locale: 'en' | 'zh',
): Promise<string> {
  const date = new Date(dateStr + 'T00:00:00')
  const from = new Date(date)
  from.setHours(dayStartHour, 0, 0, 0)
  const to = new Date(from)
  to.setDate(to.getDate() + 1)

  const range = { from, to }
  const controller = new AbortController()

  const [usageResult, commitResult] = await Promise.allSettled([
    fetchUsageHistory(range, controller.signal),
    fetchCommitHistory(range, controller.signal),
  ])

  const runs = usageResult.status === 'fulfilled' ? usageResult.value.runs : []
  const buckets = usageResult.status === 'fulfilled' ? usageResult.value.usageBuckets : []
  const commits = commitResult.status === 'fulfilled' ? commitResult.value.commits : []

  const summary = buildDailySummary(date, runs, buckets, commits, dayStartHour)

  if (mode === 'basic') {
    return buildBasicReport(summary, locale)
  }

  try {
    const prompt = buildAiPrompt(summary)
    const res = await apiFetch('/api/daily-summary/generate', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ prompt, mode }),
    })
    if (res.ok) {
      const result = await res.json()
      return result.text ?? buildBasicReport(summary, locale)
    }
  } catch {
    // fall through to basic
  }
  return buildBasicReport(summary, locale)
}

export function InsightsSection() {
  const dailySummaryEnabled = useMonitorStore((s) => s.settings.dailySummaryEnabled)
  const dailySummaryMode = useMonitorStore((s) => s.settings.dailySummaryMode)
  const dailySummaryDayStart = useMonitorStore((s) => s.settings.dailySummaryDayStart)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const historicalReports = useMonitorStore((s) => s.historicalReports)
  const setHistoricalReports = useMonitorStore((s) => s.setHistoricalReports)
  const updateHistoricalReport = useMonitorStore((s) => s.updateHistoricalReport)
  const clearHistoricalReports = useMonitorStore((s) => s.clearHistoricalReports)
  const { locale, t } = useI18n()
  const [detectedTools, setDetectedTools] = useState<Set<string>>(new Set(['basic']))
  const [fromDate, setFromDate] = useState(weekAgoStr)
  const [toDate, setToDate] = useState(yesterdayStr)
  const [expandedDates, setExpandedDates] = useState<Set<string>>(new Set())
  const [copiedAll, setCopiedAll] = useState(false)
  const [conflictCount, setConflictCount] = useState(0)
  const [pendingDates, setPendingDates] = useState<string[]>([])
  const generatingRef = useRef(false)

  useEffect(() => {
    let cancelled = false
    void apiFetch('/api/installer/detect')
      .then((res) => res.json())
      .then((data: { capabilities: ToolCapability[] }) => {
        if (cancelled) return
        const detected = new Set<string>(['basic'])
        for (const cap of data.capabilities) {
          if (cap.detected) detected.add(cap.tool)
        }
        setDetectedTools(detected)
      })
      .catch(() => {})
    return () => { cancelled = true }
  }, [])

  // Async queue processor: picks up pending reports and generates them one by one.
  // Reads from store each iteration so it works even after tab switches.
  const processQueue = useCallback(async () => {
    if (generatingRef.current) return
    generatingRef.current = true

    try {
      // eslint-disable-next-line no-constant-condition
      while (true) {
        const reports = useMonitorStore.getState().historicalReports
        const next = reports.find((r) => r.status === 'pending')
        if (!next) break

        useMonitorStore.getState().updateHistoricalReport(next.date, { status: 'generating' })

        try {
          const text = await generateOneDay(
            next.date,
            useMonitorStore.getState().settings.dailySummaryDayStart,
            useMonitorStore.getState().settings.dailySummaryMode,
            locale,
          )
          useMonitorStore.getState().updateHistoricalReport(next.date, { status: 'done', text })
        } catch {
          useMonitorStore.getState().updateHistoricalReport(next.date, { status: 'error' })
        }
      }
    } finally {
      generatingRef.current = false
    }
  }, [locale])

  const executeGenerate = useCallback((dates: string[], skipDone: boolean) => {
    const existing = new Map(
      useMonitorStore.getState().historicalReports.map((r) => [r.date, r]),
    )
    const newEntries: HistoricalReportEntry[] = []
    for (const date of dates) {
      const ex = existing.get(date)
      if (skipDone && ex && (ex.status === 'done' || ex.status === 'generating')) {
        newEntries.push(ex)
      } else {
        newEntries.push({ date, status: 'pending' })
      }
    }
    // Keep entries outside the new range
    for (const [date, entry] of existing) {
      if (!dates.includes(date)) {
        newEntries.push(entry)
      }
    }
    newEntries.sort((a, b) => a.date.localeCompare(b.date))
    setHistoricalReports(newEntries)
    setConflictCount(0)
    setPendingDates([])
    void processQueue()
  }, [setHistoricalReports, processQueue])

  const handleGenerate = useCallback(() => {
    const dates = datesInRange(fromDate, toDate)
    if (dates.length === 0) return

    // Check if any dates in range already have reports
    const existing = new Map(
      useMonitorStore.getState().historicalReports.map((r) => [r.date, r]),
    )
    const doneInRange = dates.filter((d) => {
      const ex = existing.get(d)
      return ex && ex.status === 'done'
    })

    if (doneInRange.length > 0) {
      // Show confirmation
      setConflictCount(doneInRange.length)
      setPendingDates(dates)
    } else {
      // No conflicts, generate directly
      executeGenerate(dates, false)
    }
  }, [fromDate, toDate, executeGenerate])

  const handleCopyAll = useCallback(() => {
    const doneReports = historicalReports
      .filter((r) => r.status === 'done' && r.text)
      .map((r) => r.text!)
      .join('\n\n---\n\n')
    if (!doneReports) return
    void navigator.clipboard.writeText(doneReports).then(() => {
      setCopiedAll(true)
      setTimeout(() => setCopiedAll(false), 1500)
    })
  }, [historicalReports])

  const toggleExpand = useCallback((date: string) => {
    setExpandedDates((prev) => {
      const next = new Set(prev)
      if (next.has(date)) next.delete(date)
      else next.add(date)
      return next
    })
  }, [])

  function isAvailable(mode: DailySummaryMode): boolean {
    return detectedTools.has(mode)
  }

  const doneCount = historicalReports.filter((r) => r.status === 'done').length
  const totalCount = historicalReports.length
  const isRunning = historicalReports.some((r) => r.status === 'pending' || r.status === 'generating')

  return (
    <section className="settings-section">
      <div className="section-label">{t('tab.heatmap')}</div>
      <div className="settings-cards-2">
        <div className="settings-card">
          <h3>{t('settings.dailySummary')}</h3>
          <div className="settings-toggles">
            <div className="settings-toggle-row">
              <div>
                <div className="toggle-label">{t('settings.dailySummary')}</div>
                <div className="toggle-hint">{t('settings.dailySummaryHint')}</div>
              </div>
              <button
                className={`toggle-switch ${dailySummaryEnabled ? 'on' : ''}`}
                onClick={() => updateSettings({ dailySummaryEnabled: !dailySummaryEnabled })}
                role="switch"
                aria-checked={dailySummaryEnabled}
                aria-label={t('settings.dailySummary')}
              >
                <span className="toggle-thumb" />
              </button>
            </div>
          </div>

          {dailySummaryEnabled && (
            <>
              <h3 className="settings-subsection-title">{t('settings.dailySummaryMode')}</h3>
              <p className="settings-hint">{t('settings.dailySummaryModeHint')}</p>
              <div className="settings-grid-3">
                {modes.map((mode) => {
                  const available = isAvailable(mode)
                  return (
                    <button
                      key={mode}
                      className={`settings-option ${dailySummaryMode === mode ? 'active' : ''}`}
                      disabled={!available}
                      onClick={() => updateSettings({ dailySummaryMode: mode })}
                    >
                      <span>{t(`settings.dailySummaryMode.${mode}`)}</span>
                      {!available && (
                        <span className="daily-mode-unavailable">
                          {t('settings.dailySummaryMode.unavailable')}
                        </span>
                      )}
                    </button>
                  )
                })}
              </div>

              <h3 className="settings-subsection-title">{t('settings.dayBoundary')}</h3>
              <p className="settings-hint">{t('settings.dayBoundaryHint')}</p>
              <div className="settings-grid-3">
                {dayStartOptions.map((hour) => (
                  <button
                    key={hour}
                    className={`settings-option small ${dailySummaryDayStart === hour ? 'active' : ''}`}
                    onClick={() => updateSettings({ dailySummaryDayStart: hour })}
                  >
                    {pad2(hour)}:00
                  </button>
                ))}
              </div>
            </>
          )}
        </div>

        {dailySummaryEnabled && (
          <div className="settings-card">
            <h3>{t('settings.historicalReport')}</h3>
            <p className="settings-hint">{t('settings.historicalReportHint')}</p>

            <div className="historical-range-row">
              <label className="historical-range-label">{t('settings.historicalFrom')}</label>
              <input
                type="date"
                className="historical-date-input"
                aria-label={t('settings.historicalFrom')}
                value={fromDate}
                max={todayStr()}
                onChange={(e) => setFromDate(e.target.value)}
              />
              <label className="historical-range-label">{t('settings.historicalTo')}</label>
              <input
                type="date"
                className="historical-date-input"
                aria-label={t('settings.historicalTo')}
                value={toDate}
                max={todayStr()}
                onChange={(e) => setToDate(e.target.value)}
              />
            </div>

            {conflictCount > 0 && (
              <div className="historical-conflict-bar">
                <span className="historical-conflict-msg">
                  {t('settings.historicalConflict').replace('{count}', String(conflictCount))}
                </span>
                <button
                  type="button"
                  className="settings-option active historical-conflict-btn"
                  onClick={() => executeGenerate(pendingDates, false)}
                >
                  {t('settings.historicalRegenAll')}
                </button>
                <button
                  type="button"
                  className="settings-option active historical-conflict-btn"
                  onClick={() => executeGenerate(pendingDates, true)}
                >
                  {t('settings.historicalSkipDone')}
                </button>
                <button
                  type="button"
                  className="daily-summary-btn historical-conflict-btn"
                  onClick={() => { setConflictCount(0); setPendingDates([]) }}
                >
                  {t('settings.historicalCancel')}
                </button>
              </div>
            )}

            <div className="historical-actions-row">
              <button
                type="button"
                className="settings-option active historical-generate-btn"
                onClick={handleGenerate}
                disabled={isRunning || !fromDate || !toDate || conflictCount > 0}
              >
                {isRunning ? t('settings.historicalGenerating') : t('settings.historicalGenerate')}
              </button>
              {totalCount > 0 && (
                <>
                  <span className="historical-progress">
                    {t('settings.historicalProgress')
                      .replace('{done}', String(doneCount))
                      .replace('{total}', String(totalCount))}
                  </span>
                  <button
                    type="button"
                    className="daily-summary-btn"
                    onClick={handleCopyAll}
                    disabled={doneCount === 0}
                  >
                    {copiedAll ? t('settings.historicalCopied') : t('settings.historicalCopyAll')}
                  </button>
                  <button
                    type="button"
                    className="daily-summary-btn"
                    onClick={clearHistoricalReports}
                    disabled={isRunning}
                  >
                    {t('settings.historicalClear')}
                  </button>
                </>
              )}
            </div>

            {historicalReports.length > 0 && (
              <div className="historical-report-list">
                {historicalReports.map((entry) => {
                  const expanded = expandedDates.has(entry.date)
                  return (
                    <div key={entry.date} className="historical-report-item">
                      <button
                        type="button"
                        className="historical-report-item-head"
                        onClick={() => entry.status === 'done' && toggleExpand(entry.date)}
                        disabled={entry.status !== 'done'}
                      >
                        <span className="historical-report-item-date">{entry.date}</span>
                        <span className={`historical-report-item-status status-${entry.status}`}>
                          {entry.status === 'pending' && t('settings.historicalPending')}
                          {entry.status === 'generating' && t('settings.historicalGenerating')}
                          {entry.status === 'done' && (expanded ? t('settings.historicalCollapse') : t('settings.historicalExpand'))}
                          {entry.status === 'error' && t('settings.historicalError')}
                        </span>
                      </button>
                      {expanded && entry.text && (
                        <pre className="historical-report-text">{entry.text}</pre>
                      )}
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        )}
      </div>
    </section>
  )
}

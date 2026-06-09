import { useEffect, useMemo, useState } from 'react'
import { useMonitorStore } from '../../store/monitorStore'
import { useI18n } from '../../lib/i18n'
import { buildUsageDateRange } from '../../lib/dateRange'
import { formatCost, formatTokens, getGroupKey } from '../../lib/format'
import { createHistorySelection, fetchUsageHistory, type DataMode } from '../../lib/history'
import { buildSnapshotRange, isSnapshotWindowClamped } from '../../lib/snapshotWindow'
import { collectRunUsageSlices, hasUsage, sumUsageSlices } from '../../lib/usage'
import { FixedSizeVirtualList } from '../FixedSizeVirtualList'
import { DataModeSwitch } from './DataModeSwitch'
import { SnapshotWindowSwitch } from './SnapshotWindowSwitch'
import { UsageSkeleton } from './Skeleton'
import type { ToolKind, UsageHistoryPayload } from '../../lib/types'
import { DateRangePicker } from './DateRangePicker'

import { allTools, sourceLabelsUpper as sourceLabels } from '../../lib/constants'

function accentClass(tool: ToolKind): string {
  if (tool === 'openClaw') return 'openclaw'
  return tool
}
const sourceTagLabels: Record<ToolKind, string> = {
  claude: 'Project',
  codex: 'Project',
  openClaw: 'Agent',
  hermes: 'Profile',
}
const historyPresets = ['7d', '30d', '90d', '180d'] as const

const barColors: Record<ToolKind, string> = {
  claude: 'var(--warn)',
  codex: 'var(--accent)',
  openClaw: 'var(--openclaw-accent)',
  hermes: 'var(--hermes-accent)',
}

interface GroupedUsage {
  tool: ToolKind
  totalTokens: number
  totalCost: number
  items: { tag: string; tokens: number; cost: number }[]
}

export function UsageView() {
  const data = useMonitorStore((s) => s.data)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const agentDisplayFormat = useMonitorStore((s) => s.settings.agentDisplayFormat)
  const snapshotWindow = useMonitorStore((s) => s.settings.snapshotWindow)
  const updateSettings = useMonitorStore((s) => s.updateSettings)
  const { t } = useI18n()
  const [mode, setMode] = useState<DataMode>('snapshot')
  const [historyRange, setHistoryRange] = useState(() => createHistorySelection(30))
  const [historyData, setHistoryData] = useState<UsageHistoryPayload | null>(null)
  const [historyLoading, setHistoryLoading] = useState(false)
  const [historyError, setHistoryError] = useState(false)

  useEffect(() => {
    if (mode !== 'history') return

    const controller = new AbortController()
    setHistoryLoading(true)
    setHistoryError(false)

    void fetchUsageHistory(historyRange, controller.signal)
      .then((payload) => {
        setHistoryData(payload)
      })
      .catch((error) => {
        if (error instanceof DOMException && error.name === 'AbortError') {
          return
        }
        setHistoryError(true)
      })
      .finally(() => {
        if (!controller.signal.aborted) {
          setHistoryLoading(false)
        }
      })

    return () => controller.abort()
  }, [mode, historyRange])

  const snapshotRange = useMemo(
    () => (data ? buildUsageDateRange(data.runs, data.usageBuckets) : null),
    [data],
  )

  const activeRuns = (mode === 'history' ? historyData?.runs : data?.runs) ?? []
  const activeBuckets = (mode === 'history' ? historyData?.usageBuckets : data?.usageBuckets) ?? []

  const effectiveRange = useMemo(() => {
    if (mode === 'history') {
      return historyRange
    }
    return buildSnapshotRange(snapshotWindow, snapshotRange) ?? createHistorySelection(1)
  }, [historyRange, mode, snapshotRange, snapshotWindow])

  const runUsageSlices = useMemo(
    () => collectRunUsageSlices(
      activeRuns,
      activeBuckets,
      effectiveRange.from.getTime(),
      effectiveRange.to.getTime(),
    ),
    [activeBuckets, activeRuns, effectiveRange],
  )

  const grouped = useMemo((): GroupedUsage[] => {
    const map: Record<ToolKind, GroupedUsage> = {
      claude: { tool: 'claude', totalTokens: 0, totalCost: 0, items: [] },
      codex: { tool: 'codex', totalTokens: 0, totalCost: 0, items: [] },
      openClaw: { tool: 'openClaw', totalTokens: 0, totalCost: 0, items: [] },
      hermes: { tool: 'hermes', totalTokens: 0, totalCost: 0, items: [] },
    }

    const tagMap: Record<string, Record<string, { tokens: number; cost: number }>> = {}
    for (const { run, usage } of runUsageSlices) {
      if (!hasUsage(usage)) continue
      const tool = run.tool
      if (!tagMap[tool]) tagMap[tool] = {}
      const tag = getGroupKey(run, agentDisplayFormat)
      if (!tagMap[tool][tag]) tagMap[tool][tag] = { tokens: 0, cost: 0 }
      tagMap[tool][tag].tokens += usage.totalTokens
      tagMap[tool][tag].cost += usage.costUsd ?? 0
    }

    for (const tool of allTools) {
      const entries = tagMap[tool] ?? {}
      const items = Object.entries(entries)
        .map(([tag, val]) => ({ tag, tokens: val.tokens, cost: val.cost }))
        .sort((a, b) => b.tokens - a.tokens)
      map[tool].items = items
      map[tool].totalTokens = items.reduce((sum, item) => sum + item.tokens, 0)
      map[tool].totalCost = items.reduce((sum, item) => sum + item.cost, 0)
    }

    return allTools.map((tool) => map[tool])
  }, [agentDisplayFormat, runUsageSlices])

  const totals = useMemo(() => {
    const meteredSummary = sumUsageSlices(
      runUsageSlices
        .filter(({ usage }) => hasUsage(usage))
        .map(({ usage }) => usage),
    )
    const inputs = runUsageSlices.reduce((sum, { usage }) => sum + usage.messageCount, 0)
    const items = grouped.reduce((sum, group) => sum + group.items.length, 0)
    return {
      tokens: meteredSummary.totalTokens,
      cost: meteredSummary.costUsd ?? 0,
      inputs,
      items,
    }
  }, [grouped, runUsageSlices])

  const allItems = useMemo(() => {
    const list: { tag: string; tool: ToolKind; tokens: number; cost: number }[] = []
    for (const group of grouped) {
      for (const item of group.items) {
        list.push({ tag: item.tag, tool: group.tool, tokens: item.tokens, cost: item.cost })
      }
    }
    return list.sort((a, b) => b.tokens - a.tokens)
  }, [grouped])

  if (mode === 'snapshot' && !data) {
    if (connectionStatus === 'connecting') return <UsageSkeleton />

    return (
      <div className="usage-view usage-view-empty">
        <div className="status-notice offline">
          <strong>{t('usage.offlineTitle')}</strong>
          <span>{t('usage.offlineHint')}</span>
        </div>
      </div>
    )
  }

  const snapshotDays = data?.config.historyDays ?? 7
  const snapshotClampHint = mode === 'snapshot' && isSnapshotWindowClamped(snapshotWindow, snapshotDays)
    ? t('history.snapshotClampHint').replace('{days}', String(snapshotDays))
    : null

  return (
    <div className="usage-view">
      {mode === 'snapshot' && connectionStatus === 'offline' && (
        <div className="status-notice offline">
          <strong>{t('usage.offlineTitle')}</strong>
          <span>{t('usage.offlineHint')}</span>
        </div>
      )}

      <div className="history-toolbar">
        <div className="history-toolbar-main">
          <DataModeSwitch mode={mode} onChange={setMode} />
          {snapshotClampHint && (
            <span className="history-toolbar-subhint">{snapshotClampHint}</span>
          )}
        </div>
        {mode === 'snapshot' ? (
          <SnapshotWindowSwitch
            value={snapshotWindow}
            onChange={(nextWindow) => updateSettings({ snapshotWindow: nextWindow })}
          />
        ) : (
          <DateRangePicker
            value={historyRange}
            onChange={setHistoryRange}
            presets={historyPresets}
          />
        )}
      </div>

      {mode === 'history' && historyLoading && (
        <div className="status-notice">
          <strong>{t('history.loading')}</strong>
        </div>
      )}
      {mode === 'history' && historyError && (
        <div className="status-notice offline">
          <strong>{t('history.error')}</strong>
        </div>
      )}
      {mode === 'history' && historyData?.truncated && (
        <div className="status-notice warn">
          <strong>{t('history.truncated')}</strong>
        </div>
      )}

      <div className="usage-top-strip">
        <div className="usage-totals">
          <div className="usage-total-item summary-stat">
            <span className="summary-label">{t('usage.totalTokens')}</span>
            <strong className="summary-value">{formatTokens(totals.tokens)}</strong>
          </div>
          <div className="usage-total-item summary-stat">
            <span className="summary-label">{t('usage.totalCost')}</span>
            <strong className="summary-value">{formatCost(totals.cost)}</strong>
          </div>
          <div className="usage-total-item summary-stat">
            <span className="summary-label">{t('usage.totalInputs')}</span>
            <strong className="summary-value">{Math.round(totals.inputs).toLocaleString()}</strong>
          </div>
          <div className="usage-total-item summary-stat">
            <span className="summary-label">{t('usage.items')}</span>
            <strong className="summary-value">{totals.items}</strong>
          </div>
        </div>
      </div>

      <section className="page-section">
        <div className="usage-section-label">{t('usage.bySource')}</div>
        <div className="usage-source-columns">
          {grouped.map((group) => {
            const maxTokens = Math.max(1, ...group.items.map((item) => item.tokens))
            return (
              <div key={group.tool} className={`usage-source-card accent-${accentClass(group.tool)}`}>
                <div className="usage-source-header">
                  <span className="usage-source-name">{sourceLabels[group.tool]}</span>
                  <span className="usage-source-tag-type">{sourceTagLabels[group.tool]}</span>
                </div>
                <div className="usage-source-totals">
                  {formatTokens(group.totalTokens)} tokens &nbsp; {formatCost(group.totalCost)}
                </div>
                <div
                  className="usage-source-items"
                  role="region"
                  tabIndex={0}
                  aria-label={`${sourceLabels[group.tool]} ${t('usage.bySource').toLowerCase()}`}
                >
                  {group.items.map((item) => (
                    <div key={item.tag} className="usage-item">
                      <div className="usage-item-row">
                        <span className="usage-item-tag">{item.tag}</span>
                        <span className="usage-item-tokens">{formatTokens(item.tokens)}</span>
                      </div>
                      <div className="usage-item-bar">
                        <span
                          className="usage-item-fill"
                          style={{
                            width: `${(item.tokens / maxTokens) * 100}%`,
                            background: barColors[group.tool],
                          }}
                        />
                      </div>
                      <span className="usage-item-cost">{formatCost(item.cost)}</span>
                    </div>
                  ))}
                </div>
              </div>
            )
          })}
        </div>
      </section>

      <section className="page-section">
        <div className="usage-section-label">{t('usage.allItems')}</div>
        {allItems.length === 0 ? (
          <div className="empty-state-panel">
            <strong>{t('usage.noItems')}</strong>
            <span>{t('usage.sessionTotals')}</span>
          </div>
        ) : (
          <FixedSizeVirtualList
            items={allItems}
            className="usage-all-items"
            tabIndex={0}
            ariaLabel={t('usage.allItems')}
            itemHeight={84}
            threshold={36}
            getKey={(item) => `${item.tool}-${item.tag}`}
            renderItem={(item) => (
              <div className={`usage-all-row accent-${accentClass(item.tool)}-left`}>
                <div className="usage-all-info">
                  <span className="usage-all-tag">{item.tag}</span>
                  <span className="usage-all-source">{sourceLabels[item.tool]}</span>
                </div>
                <div className="usage-all-numbers">
                  <span className="usage-all-tokens">{formatTokens(item.tokens)}</span>
                  <span className="usage-all-cost">{formatCost(item.cost)}</span>
                </div>
              </div>
            )}
          />
        )}
      </section>
    </div>
  )
}

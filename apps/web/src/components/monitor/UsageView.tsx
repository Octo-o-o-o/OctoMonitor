import { useEffect, useMemo, useState } from 'react'
import { useMonitorStore } from '../../store/monitorStore'
import { useI18n } from '../../lib/i18n'
import { buildUsageDateRange, endOfDay, startOfDay } from '../../lib/dateRange'
import { formatTokens, formatCost, getGroupKey } from '../../lib/format'
import { collectRunUsageSlices, hasUsage, sumUsageSlices } from '../../lib/usage'
import { UsageSkeleton } from './Skeleton'
import type { ToolKind } from '../../lib/types'
import { DateRangePicker, type DateRange } from './DateRangePicker'

const sourceOrder: ToolKind[] = ['claude', 'codex', 'openClaw']
const sourceLabels: Record<ToolKind, string> = {
  claude: 'CLAUDE CODE',
  codex: 'CODEX',
  openClaw: 'OPENCLAW',
}
const sourceTagLabels: Record<ToolKind, string> = {
  claude: 'Project',
  codex: 'Project',
  openClaw: 'Agent',
}


function getBarColor(tool: ToolKind): string {
  switch (tool) {
    case 'claude': return 'var(--warn)'
    case 'codex': return 'var(--accent)'
    case 'openClaw': return 'var(--openclaw-accent)'
    default: return 'var(--accent)'
  }
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
  const { t } = useI18n()
  const [dateRange, setDateRange] = useState<DateRange>(() => {
    const to = endOfDay(new Date())
    const from = startOfDay(new Date())
    return { from, to }
  })
  const [dateRangeLocked, setDateRangeLocked] = useState(false)

  const allRange = useMemo(
    () => (data ? buildUsageDateRange(data.runs, data.usageBuckets) : null),
    [data],
  )

  useEffect(() => {
    if (!allRange || dateRangeLocked) return
    setDateRange(allRange)
  }, [allRange, dateRangeLocked])

  const runUsageSlices = useMemo(() => {
    if (!data) return []
    return collectRunUsageSlices(
      data.runs,
      data.usageBuckets,
      dateRange.from.getTime(),
      dateRange.to.getTime(),
    )
  }, [data, dateRange])

  function handleDateRangeChange(nextRange: DateRange) {
    setDateRangeLocked(true)
    setDateRange(nextRange)
  }

  const grouped = useMemo((): GroupedUsage[] => {
    if (!data) return []
    const map: Record<ToolKind, GroupedUsage> = {
      claude: { tool: 'claude', totalTokens: 0, totalCost: 0, items: [] },
      codex: { tool: 'codex', totalTokens: 0, totalCost: 0, items: [] },
      openClaw: { tool: 'openClaw', totalTokens: 0, totalCost: 0, items: [] },
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

    for (const tool of sourceOrder) {
      const entries = tagMap[tool] ?? {}
      const items = Object.entries(entries)
        .map(([tag, val]) => ({ tag, tokens: val.tokens, cost: val.cost }))
        .sort((a, b) => b.tokens - a.tokens)
      map[tool].items = items
      map[tool].totalTokens = items.reduce((s, i) => s + i.tokens, 0)
      map[tool].totalCost = items.reduce((s, i) => s + i.cost, 0)
    }

    return sourceOrder.map((tool) => map[tool])
  }, [data, agentDisplayFormat, runUsageSlices])

  const totals = useMemo(() => {
    const summary = sumUsageSlices(
      runUsageSlices
        .filter(({ usage }) => hasUsage(usage))
        .map(({ usage }) => usage),
    )
    const items = grouped.reduce((s, g) => s + g.items.length, 0)
    return {
      tokens: summary.totalTokens,
      cost: summary.costUsd ?? 0,
      items,
    }
  }, [grouped, runUsageSlices])

  const allItems = useMemo(() => {
    const list: { tag: string; tool: ToolKind; tokens: number; cost: number }[] = []
    for (const g of grouped) {
      for (const item of g.items) {
        list.push({ tag: item.tag, tool: g.tool, tokens: item.tokens, cost: item.cost })
      }
    }
    return list.sort((a, b) => b.tokens - a.tokens)
  }, [grouped])

  if (!data) {
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

  return (
    <div className="usage-view">
      {connectionStatus === 'offline' && (
        <div className="status-notice offline">
          <strong>{t('usage.offlineTitle')}</strong>
          <span>{t('usage.offlineHint')}</span>
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
            <span className="summary-label">{t('usage.items')}</span>
            <strong className="summary-value">{totals.items}</strong>
          </div>
        </div>
        <DateRangePicker value={dateRange} onChange={handleDateRangeChange} allRange={allRange} />
      </div>

      <section className="page-section">
        <div className="usage-section-label">{t('usage.bySource')}</div>
        <div className="usage-source-columns">
          {grouped.map((group) => {
            const maxTokens = Math.max(1, ...group.items.map((i) => i.tokens))
            return (
              <div key={group.tool} className={`usage-source-card accent-${group.tool === 'openClaw' ? 'openclaw' : group.tool}`}>
                <div className="usage-source-header">
                  <span className="usage-source-name">{sourceLabels[group.tool]}</span>
                  <span className="usage-source-tag-type">{sourceTagLabels[group.tool]}</span>
                </div>
                <div className="usage-source-totals">
                  {formatTokens(group.totalTokens)} tokens &nbsp; {formatCost(group.totalCost)}
                </div>
                <div
                  className="usage-source-items"
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
                            background: getBarColor(group.tool),
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
          <div className="usage-all-items" tabIndex={0} aria-label={t('usage.allItems')}>
            {allItems.map((item) => (
              <div key={`${item.tool}-${item.tag}`} className={`usage-all-row accent-${item.tool === 'openClaw' ? 'openclaw' : item.tool}-left`}>
                <div className="usage-all-info">
                  <span className="usage-all-tag">{item.tag}</span>
                  <span className="usage-all-source">{sourceLabels[item.tool]}</span>
                </div>
                <div className="usage-all-numbers">
                  <span className="usage-all-tokens">{formatTokens(item.tokens)}</span>
                  <span className="usage-all-cost">{formatCost(item.cost)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  )
}

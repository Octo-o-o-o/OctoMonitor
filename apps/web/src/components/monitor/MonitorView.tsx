import { useMemo } from 'react'
import { useMonitorStore, type AgentDisplayFormat } from '../../store/monitorStore'
import { useI18n, type I18nKey } from '../../lib/i18n'
import { stateLabelKeys } from '../../lib/i18nMaps'
import { formatTokens, formatDuration, formatLastUpdated, formatAgentHandle } from '../../lib/format'
import {
  buildVisiblePanels,
  buildVisibleRunsBySource,
  flattenVisibleRunsBySource,
  getMonitorTaskSection,
  type MonitorTaskSection,
} from '../../lib/monitor'
import { applyMonitorFilters } from '../../lib/monitorFilters'
import { AttentionBanner } from './AttentionBanner'
import { MonitorFilterBar } from './MonitorFilterBar'
import { MonitorSkeleton } from './Skeleton'
import type { AdapterHealth, PendingCron, RunRecord, ToolKind } from '../../lib/types'

import { sourceLabels } from '../../lib/constants'

const sourceAccents: Partial<Record<ToolKind, string>> = {
  claude: 'accent-claude',
  codex: 'accent-codex',
  openClaw: 'accent-openclaw',
  hermes: 'accent-hermes',
}

const stateStyles: Record<string, { badge: string; row: string }> = {
  active: { badge: 'state-running', row: 'state-active' },
  waitingApproval: { badge: 'state-waiting', row: 'state-waiting' },
  completed: { badge: 'state-done', row: 'state-done' },
  idle: { badge: 'state-done', row: 'state-done' },
  error: { badge: 'state-error', row: 'state-error' },
  stale: { badge: 'state-done', row: 'state-done' },
  gatewayOffline: { badge: 'state-error', row: 'state-error' },
  limitExceeded: { badge: 'state-error', row: 'state-error' },
  contextExceeded: { badge: 'state-error', row: 'state-error' },
  cancelled: { badge: 'state-done', row: 'state-done' },
}

const defaultStateStyle = { badge: 'state-done', row: 'state-done' }

const taskSections: Array<{ key: MonitorTaskSection; labelKey: I18nKey }> = [
  { key: 'attention', labelKey: 'monitor.section.attention' },
  { key: 'active', labelKey: 'monitor.section.active' },
  { key: 'error', labelKey: 'monitor.section.error' },
  { key: 'done', labelKey: 'monitor.section.done' },
]

function getSourceIndicator(health: AdapterHealth | undefined) {
  if (health?.gatewayStatus) {
    return {
      dotClass: health.gatewayStatus,
      labelKey: `monitor.gateway.${health.gatewayStatus}` as const,
    }
  }
  return {
    dotClass: health?.online ? 'online' : 'offline',
    labelKey: undefined,
  }
}

function firstMeaningfulText(...values: Array<string | null | undefined>): string {
  return values
    .map((value) => value?.trim())
    .find((value): value is string => Boolean(value))
    ?? ''
}

function runTitle(run: RunRecord): string {
  return firstMeaningfulText(
    run.lastQuestion,
    run.firstQuestion,
    run.lastAction,
    run.projectName,
    sourceLabels[run.tool],
  )
}

function runWorkspaceLabel(run: RunRecord): string | undefined {
  return firstMeaningfulText(run.workspaceShort, run.workspacePath) || undefined
}

function TaskRow({
  run,
  focused,
  onClick,
}: {
  run: RunRecord
  focused?: boolean
  onClick: () => void
}) {
  const acknowledgedErrors = useMonitorStore((s) => s.acknowledgedErrors)
  const acknowledgeError = useMonitorStore((s) => s.acknowledgeError)
  const visitedRunIds = useMonitorStore((s) => s.visitedRunIds)
  const markRunVisited = useMonitorStore((s) => s.markRunVisited)
  const { t } = useI18n()

  const stateLabel = t(stateLabelKeys[run.state])
  const style = stateStyles[run.state] ?? defaultStateStyle
  const isError = style.row === 'state-error'
  const isAcknowledged = isError && acknowledgedErrors.has(run.id)
  const supportsVisitedVisual = run.state === 'completed'
    || run.state === 'idle'
    || run.state === 'stale'
    || run.state === 'cancelled'
  const isVisitedDone = supportsVisitedVisual && visitedRunIds.has(run.id)
  const isUnvisitedDone = supportsVisitedVisual && !isVisitedDone
  const rowClass = isAcknowledged ? 'state-error-ack' : style.row
  const workspace = runWorkspaceLabel(run)
  const isWaiting = run.state === 'waitingApproval'

  const handleClick = () => {
    if (isError && !isAcknowledged) {
      acknowledgeError(run.id)
    }
    markRunVisited(run.id)
    onClick()
  }

  return (
    <button
      className={`task-feed-row ${rowClass}${focused ? ' session-focused' : ''}${isVisitedDone ? ' is-visited' : ''}`}
      data-run-id={run.id}
      onClick={handleClick}
    >
      <span className={`task-status-stripe ${style.badge}`} aria-hidden="true" />
      <span className="task-feed-main">
        <span className="task-feed-title-row">
          <span className={`state-badge ${style.badge}`}>{stateLabel}</span>
          {isUnvisitedDone && <span className="session-unvisited-dot" aria-hidden="true" />}
          <span className="task-feed-title">{runTitle(run)}</span>
        </span>
        <span className="task-feed-meta">
          <span>{run.projectName}</span>
          {workspace && (
            <>
              <span className="task-feed-sep">·</span>
              <span>{workspace}</span>
            </>
          )}
          <span className={`task-tool-pill ${sourceAccents[run.tool] ?? 'accent-generic'}`}>
            {sourceLabels[run.tool]}
          </span>
          {run.model && <span className="task-model-pill">{run.model}</span>}
        </span>
        {(run.lastTail || run.errorMessage) && (
          <span className={`task-feed-tail${isWaiting ? ' urgent' : ''}`}>
            {run.errorMessage ?? run.lastTail}
          </span>
        )}
      </span>
      <span className="task-feed-metrics">
        <span className="task-feed-metric">{formatLastUpdated(run.lastActivityAt)}</span>
        <span className="task-feed-metric">{formatDuration(run.elapsedMs)}</span>
        <span className="task-feed-metric">{formatTokens(run.tokens.total)}</span>
      </span>
    </button>
  )
}

function groupRunsByTaskSection(runs: RunRecord[]): Record<MonitorTaskSection, RunRecord[]> {
  return runs.reduce<Record<MonitorTaskSection, RunRecord[]>>((groups, run) => {
    groups[getMonitorTaskSection(run)].push(run)
    return groups
  }, {
    attention: [],
    active: [],
    error: [],
    done: [],
  })
}

function cronParseField(field: string, max: number): number[] {
  if (field === '*') return Array.from({ length: max }, (_, i) => i)
  if (field.startsWith('*/')) {
    const step = parseInt(field.slice(2))
    return Array.from({ length: Math.ceil(max / step) }, (_, i) => i * step)
  }
  return field.split(',').map(Number)
}

const dowNames = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']

type ExpandedCron = PendingCron & { _fireMinutes: number }
function expandCrons(crons: PendingCron[]): ExpandedCron[] {
  const now = new Date()
  const nowMin = now.getHours() * 60 + now.getMinutes()
  const nowDow = now.getDay()
  const result: ExpandedCron[] = []

  for (const cron of crons) {
    const parts = cron.scheduleExpr.split(' ')
    if (parts.length !== 5) {
      result.push({ ...cron, _fireMinutes: Infinity })
      continue
    }
    const [minPart, hourPart, , , dowPart] = parts
    const minutes = cronParseField(minPart, 60)
    const hours = cronParseField(hourPart, 24)
    const dows = dowPart === '*' ? null : dowPart.split(',').map(Number)

    const combos: { h: number; m: number }[] = []
    for (const h of hours) for (const m of minutes) combos.push({ h, m })

    const needsExpand = combos.length > 1 && (hours.length > 1 || minutes.length > 1)
      && !hourPart.startsWith('*') && !minPart.startsWith('*')

    if (!needsExpand) {
      const times = combos.map(({ h, m }) => h * 60 + m).sort((a, b) => a - b)
      let fire: number
      if (!dows) {
        const next = times.find((t) => t > nowMin)
        fire = next !== undefined ? next - nowMin : 1440 - nowMin + times[0]
      } else {
        fire = Infinity
        for (const dow of dows) {
          for (const t of times) {
            let dd = (dow - nowDow + 7) % 7
            if (dd === 0 && t <= nowMin) dd = 7
            const delta = dd * 1440 + (t - nowMin)
            if (delta > 0 && delta < fire) fire = delta
          }
        }
      }
      result.push({ ...cron, _fireMinutes: fire })
      continue
    }

    const prefix = dows
      ? dows.map((d) => dowNames[d]).join(',')
      : 'Daily'

    for (const { h, m } of combos) {
      const timeStr = `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}`
      const humanStr = `${prefix} ${timeStr}`
      const singleExpr = `${m} ${h} ${parts[2]} ${parts[3]} ${parts[4]}`
      const todayMin = h * 60 + m
      let fire: number
      if (!dows) {
        fire = todayMin > nowMin ? todayMin - nowMin : 1440 - nowMin + todayMin
      } else {
        fire = Infinity
        for (const dow of dows) {
          let dd = (dow - nowDow + 7) % 7
          if (dd === 0 && todayMin <= nowMin) dd = 7
          const delta = dd * 1440 + (todayMin - nowMin)
          if (delta > 0 && delta < fire) fire = delta
        }
      }
      result.push({
        ...cron,
        id: `${cron.id}__${h}_${m}`,
        scheduleExpr: singleExpr,
        scheduleHuman: humanStr,
        _fireMinutes: fire,
      })
    }
  }

  return result.sort((a, b) => a._fireMinutes - b._fireMinutes)
}

function formatScheduleHuman(s: string): string {
  let r = s.replace(/(\d{1,2}:\d{2}):\d{2}/g, '$1')
  r = r.replace(/^(Mon|Tue|Wed|Thu|Fri|Sat|Sun)\s/, (_, day: string) => day.padEnd(6))
  return r
}

function formatCronAgent(
  cron: PendingCron,
  format: AgentDisplayFormat,
  nameMap: Map<string, string>,
): string | undefined {
  if (!cron.agentId) return undefined
  return formatAgentHandle(cron.agentId, cron.agentDisplayName ?? nameMap.get(cron.agentId), format)
}

type SourceIssue = {
  tool: ToolKind
  severity: 'warn' | 'error'
  title: string
  detail: string
}

function buildSourceIssues(
  healthRows: AdapterHealth[],
  visibleTools: ToolKind[],
  t: (key: I18nKey) => string,
): SourceIssue[] {
  const visible = new Set(visibleTools)
  const issues: SourceIssue[] = []

  for (const health of healthRows) {
    if (!visible.has(health.tool)) continue
    const sourceName = sourceLabels[health.tool]
    const indicator = getSourceIndicator(health)
    const gatewayLabel = indicator.labelKey ? t(indicator.labelKey) : undefined

    if (!health.online) {
      issues.push({
        tool: health.tool,
        severity: 'error',
        title: `${sourceName} ${t('monitor.issue.offline')}`,
        detail: health.lastError ?? health.gatewayDetail ?? health.mode,
      })
      continue
    }

    if (health.gatewayStatus && health.gatewayStatus !== 'running') {
      issues.push({
        tool: health.tool,
        severity: health.gatewayStatus === 'stopped' ? 'error' : 'warn',
        title: `${sourceName} ${gatewayLabel ?? health.gatewayStatus}`,
        detail: health.gatewayDetail ?? health.mode,
      })
      continue
    }

    if (health.lastError) {
      issues.push({
        tool: health.tool,
        severity: 'warn',
        title: `${sourceName} ${t('monitor.issue.warning')}`,
        detail: health.lastError,
      })
    }
  }

  return issues
}

function PendingCronList({ crons, runs }: { crons: PendingCron[]; runs: RunRecord[] }) {
  const agentDisplayFormat = useMonitorStore((s) => s.settings.agentDisplayFormat)
  const { t } = useI18n()

  const nameMap = useMemo(() => {
    const m = new Map<string, string>()
    for (const r of runs) {
      if (r.tool === 'openClaw' && r.agentName && r.agentDisplayName) {
        m.set(r.agentName, r.agentDisplayName)
      }
    }
    return m
  }, [runs])

  const sorted = useMemo(() => expandCrons(crons), [crons])
  if (sorted.length === 0) return null

  return (
    <div className="monitor-rail-section">
      <div className="monitor-rail-section-head">
        <span>{t('ui.scheduled')}</span>
        <strong>{sorted.length}</strong>
      </div>
      <div className="monitor-cron-list">
        {sorted.slice(0, 6).map((cron) => {
          const agent = formatCronAgent(cron, agentDisplayFormat, nameMap)
          return (
            <div key={cron.id} className="monitor-cron-item">
              <span className="monitor-cron-time">{formatScheduleHuman(cron.scheduleHuman)}</span>
              <strong>{cron.name}</strong>
              {agent && <span>{agent}</span>}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function MonitorRail({
  issues,
  crons,
  runs,
}: {
  issues: SourceIssue[]
  crons: PendingCron[]
  runs: RunRecord[]
}) {
  const { t } = useI18n()
  if (issues.length === 0 && crons.length === 0) return null

  return (
    <aside className="monitor-rail" aria-label={t('monitor.rail.label')}>
      {issues.length > 0 && (
        <div className="monitor-rail-section">
          <div className="monitor-rail-section-head">
            <span>{t('monitor.rail.issues')}</span>
            <strong>{issues.length}</strong>
          </div>
          <div className="monitor-issue-list">
            {issues.map((issue) => (
              <div key={`${issue.tool}-${issue.title}`} className={`monitor-issue-item ${issue.severity}`}>
                <div className="monitor-issue-title">
                  <span className={`source-dot ${issue.severity === 'error' ? 'stopped' : 'warning'}`} />
                  <strong>{issue.title}</strong>
                </div>
                <span>{issue.detail}</span>
              </div>
            ))}
          </div>
        </div>
      )}
      <PendingCronList crons={crons} runs={runs} />
    </aside>
  )
}

export function MonitorView() {
  const data = useMonitorStore((s) => s.data)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)
  const panelConfig = useMonitorStore((s) => s.settings.panelConfig)
  const filterRules = useMonitorStore((s) => s.settings.filterRules)
  const quickFilter = useMonitorStore((s) => s.monitorQuickFilter)
  const toolFilter = useMonitorStore((s) => s.monitorToolFilter)
  const searchQuery = useMonitorStore((s) => s.monitorSearch)
  const selectRun = useMonitorStore((s) => s.selectRun)
  const focusedRunId = useMonitorStore((s) => s.focusedRunId)
  const { t } = useI18n()

  const visiblePanels = useMemo(
    () => {
      const hidden = new Set(data?.config.hiddenSources ?? [])
      return buildVisiblePanels(panelConfig).filter((tool) => !hidden.has(tool))
    },
    [data?.config.hiddenSources, panelConfig],
  )

  const effectiveToolFilter = toolFilter === 'all' || visiblePanels.includes(toolFilter)
    ? toolFilter
    : 'all'

  const visibleRuns = useMemo(() => {
    if (!data) return []
    const base = buildVisibleRunsBySource(data.runs, filterRules, monitorPeriod)
    return applyMonitorFilters(
      flattenVisibleRunsBySource(base, visiblePanels),
      quickFilter,
      effectiveToolFilter,
      searchQuery,
    )
  }, [data, effectiveToolFilter, filterRules, monitorPeriod, quickFilter, searchQuery, visiblePanels])

  const groupedRuns = useMemo(() => groupRunsByTaskSection(visibleRuns), [visibleRuns])
  const issues = useMemo(
    () => data ? buildSourceIssues(data.adapterHealth, visiblePanels, t) : [],
    [data, t, visiblePanels],
  )
  const showRail = Boolean(data && (issues.length > 0 || data.pendingCrons.length > 0))

  if (!data) {
    if (connectionStatus === 'connecting') return <MonitorSkeleton />

    return (
      <div className="monitor-view monitor-view-empty">
        <div className="status-notice offline">
          <strong>{t('monitor.offlineTitle')}</strong>
          <span>{t('monitor.offlineHint')}</span>
        </div>
      </div>
    )
  }

  return (
    <div className="monitor-view">
      {connectionStatus === 'offline' && (
        <div className="status-notice offline">
          <strong>{t('monitor.offlineTitle')}</strong>
          <span>{t('monitor.offlineHint')}</span>
        </div>
      )}
      <AttentionBanner items={data.attentions} />
      <MonitorFilterBar visibleTools={visiblePanels} />
      <section className="monitor-board-panel">
        <div className={`task-feed-layout${showRail ? ' has-rail' : ''}`}>
          <article className="task-feed-board">
            <div className="task-feed-head">
              <h2>{t('monitor.taskFeed')}</h2>
              <span>{visibleRuns.length} {t('monitor.taskCount')}</span>
            </div>
            {visibleRuns.length === 0 ? (
              <div className="empty-state-panel task-feed-empty">
                <strong>{t('monitor.emptyTitle')}</strong>
                <span>{t('monitor.emptyHint')}</span>
              </div>
            ) : (
              taskSections.map((section) => {
                const runs = groupedRuns[section.key]
                if (runs.length === 0) return null
                return (
                  <section key={section.key} className="task-feed-section">
                    <div className="task-feed-section-head">
                      <span>{t(section.labelKey)}</span>
                      <strong>{runs.length}</strong>
                    </div>
                    <div className="task-feed-list">
                      {runs.map((run) => (
                        <TaskRow
                          key={run.id}
                          run={run}
                          focused={run.id === focusedRunId}
                          onClick={() => selectRun(run.id)}
                        />
                      ))}
                    </div>
                  </section>
                )
              })
            )}
          </article>
          {showRail && (
            <MonitorRail
              issues={issues}
              crons={data.pendingCrons}
              runs={data.runs}
            />
          )}
        </div>
      </section>
    </div>
  )
}

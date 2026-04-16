import { useMemo, useState } from 'react'
import { useMonitorStore, type AgentDisplayFormat } from '../../store/monitorStore'
import { useI18n } from '../../lib/i18n'
import { formatTokens, formatDuration, formatLastUpdated, formatAgentTag, getGroupKey } from '../../lib/format'
import { buildVisiblePanels, buildVisibleRunsBySource, summarizeRunsByState } from '../../lib/monitor'
import { NARROW_LAYOUT_QUERY, useMediaQuery } from '../../lib/responsive'
import { AttentionBanner } from './AttentionBanner'
import { MonitorSkeleton } from './Skeleton'
import type { PendingCron, RunRecord, ToolKind } from '../../lib/types'

import { sourceLabelsUpper as sourceLabels } from '../../lib/constants'
const sourceAccents: Record<ToolKind, string> = {
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
}

const defaultStateStyle = { badge: 'state-done', row: 'state-done' }

const tagPalette = [
  { bg: 'var(--tag-violet-bg)', text: 'var(--tag-violet-text)' },
  { bg: 'var(--tag-cyan-bg)', text: 'var(--tag-cyan-text)' },
  { bg: 'var(--tag-rose-bg)', text: 'var(--tag-rose-text)' },
  { bg: 'var(--tag-amber-bg)', text: 'var(--tag-amber-text)' },
  { bg: 'var(--tag-emerald-bg)', text: 'var(--tag-emerald-text)' },
  { bg: 'var(--tag-blue-bg)', text: 'var(--tag-blue-text)' },
  { bg: 'var(--tag-fuchsia-bg)', text: 'var(--tag-fuchsia-text)' },
  { bg: 'var(--tag-lime-bg)', text: 'var(--tag-lime-text)' },
  { bg: 'var(--tag-orange-bg)', text: 'var(--tag-orange-text)' },
  { bg: 'var(--tag-teal-bg)', text: 'var(--tag-teal-text)' },
]

function getTagColor(tag: string): { bg: string; text: string } {
  let hash = 0
  for (let i = 0; i < tag.length; i++) {
    hash = ((hash << 5) - hash) + tag.charCodeAt(i)
    hash = hash & hash
  }
  return tagPalette[Math.abs(hash) % tagPalette.length]
}

function getStateCategory(state: string): 'active' | 'waiting' | 'done' {
  if (state === 'active') return 'active'
  if (state === 'waitingApproval') return 'waiting'
  return 'done'
}

interface ProjectGroup {
  key: string
  runs: RunRecord[]
}

function groupRunsByProject(runs: RunRecord[], agentDisplayFormat: AgentDisplayFormat): { active: ProjectGroup[]; waiting: ProjectGroup[]; done: ProjectGroup[] } {
  const buckets: Record<'active' | 'waiting' | 'done', Map<string, RunRecord[]>> = {
    active: new Map(),
    waiting: new Map(),
    done: new Map(),
  }
  for (const run of runs) {
    const cat = getStateCategory(run.state)
    const key = getGroupKey(run, agentDisplayFormat)
    const list = buckets[cat].get(key)
    if (list) list.push(run)
    else buckets[cat].set(key, [run])
  }
  const toGroups = (m: Map<string, RunRecord[]>): ProjectGroup[] =>
    Array.from(m.entries()).map(([key, runs]) => ({ key, runs }))
  return { active: toGroups(buckets.active), waiting: toGroups(buckets.waiting), done: toGroups(buckets.done) }
}

function SessionRow({ run, onClick, focused, hideTag, hideBadge }: { run: RunRecord; onClick: () => void; focused?: boolean; hideTag?: boolean; hideBadge?: boolean }) {
  const agentDisplayFormat = useMonitorStore((s) => s.settings.agentDisplayFormat)
  const acknowledgedErrors = useMonitorStore((s) => s.acknowledgedErrors)
  const acknowledgeError = useMonitorStore((s) => s.acknowledgeError)
  const { t } = useI18n()
  const tag = run.tool === 'openClaw'
    ? formatAgentTag(run, agentDisplayFormat)
    : run.workspaceShort
  const stateKey = `state.${run.state}` as any
  const stateLabel = t(stateKey)
  const style = stateStyles[run.state] ?? defaultStateStyle
  const isError = style.row === 'state-error'
  const isAcknowledged = isError && acknowledgedErrors.has(run.id)
  const rowClass = isAcknowledged ? 'state-error-ack' : style.row
  const tagColor = getTagColor(tag)
  const isWaiting = run.state === 'waitingApproval'

  const originBadge = run.tool === 'openClaw' && run.originProvider && run.originProvider !== 'heartbeat'
    ? run.originLabel ?? run.originProvider
    : undefined

  const handleClick = () => {
    if (isError && !isAcknowledged) {
      acknowledgeError(run.id)
    }
    onClick()
  }

  return (
    <button className={`session-row ${rowClass}${focused ? ' session-focused' : ''}`} data-run-id={run.id} onClick={handleClick}>
      <div className="session-header">
        {!hideBadge && <span className={`state-badge ${style.badge}`}>{stateLabel}</span>}
        <span className="session-duration">{formatDuration(run.elapsedMs)}</span>
        <span className="session-updated">{formatLastUpdated(run.lastActivityAt)}</span>
        {run.tool === 'openClaw' && run.model && (
          <span className="model-badge">{run.model}</span>
        )}
        <span className="session-header-right">
          {run.messageCount > 0 && (
            <span className="session-msg-count">{run.messageCount} {t('ui.inputCount')}</span>
          )}
          <span className="session-tokens">{formatTokens(run.tokens.total)}</span>
        </span>
      </div>
      <div className="session-title">{run.lastQuestion ?? run.firstQuestion ?? run.lastAction ?? run.projectName}</div>
      <div className="session-footer">
        {!hideTag && (
          <span
            className="session-tag"
            style={{ background: tagColor.bg, color: tagColor.text }}
          >
            {tag}
          </span>
        )}
        {originBadge && (
          <span className="session-origin">{originBadge}</span>
        )}
        <span className={`session-detail${isWaiting ? ' urgent' : ''}`}>
          {run.lastTail ?? ''}
        </span>
      </div>
    </button>
  )
}

function ProjectGroupSection({ group, selectRun, focusedRunId }: { group: ProjectGroup; selectRun: (id: string) => void; focusedRunId?: string }) {
  const tagColor = getTagColor(group.key)
  const firstStyle = stateStyles[group.runs[0].state] ?? defaultStateStyle
  const uniformState = group.runs.every(
    (r) => (stateStyles[r.state] ?? defaultStateStyle).badge === firstStyle.badge,
  )
  const groupRowClass = uniformState ? firstStyle.row : ''
  const isMulti = group.runs.length > 1
  return (
    <div className={`project-group has-header ${groupRowClass}`}>
      <div className="project-group-header">
        <span
          className="session-tag"
          style={{ background: tagColor.bg, color: tagColor.text }}
        >
          {group.key}
        </span>
        {isMulti && <span className="project-group-count">{group.runs.length}</span>}
      </div>
      {group.runs.map((run) => (
        <SessionRow
          key={run.id}
          run={run}
          focused={run.id === focusedRunId}
          onClick={() => selectRun(run.id)}
          hideTag
          hideBadge={uniformState}
        />
      ))}
    </div>
  )
}

function QuotaBar({ label, pct, loading }: { label: string; pct: number | null | undefined; loading?: boolean }) {
  const color = 'var(--accent)'
  if (loading) {
    return (
      <span className="quota-bar-wrap quota-loading">
        <span className="quota-bar-inner">
          <span className="quota-label">{label}</span>
          <span className="quota-pct quota-pct-loading">—</span>
        </span>
      </span>
    )
  }
  if (pct == null) return null
  return (
    <span className="quota-bar-wrap">
      <span className="quota-fill" style={{ width: `${pct}%`, background: color }} />
      <span className="quota-bar-inner">
        <span className="quota-label">{label}</span>
        <span className="quota-pct">{pct}%</span>
      </span>
    </span>
  )
}

function PayAsYouGoBadge() {
  const { t } = useI18n()
  return (
    <div className="quota-bars">
      <span className="quota-payg">{t('ui.payAsYouGo')}</span>
    </div>
  )
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
  const id = cron.agentId
  const name = cron.agentDisplayName ?? nameMap.get(id)
  switch (format) {
    case 'name': return name ? `@${name}` : `@${id}`
    case 'id:name': return name ? `@${id}:${name}` : `@${id}`
    default: return `@${id}`
  }
}

function CronList({ crons }: { crons: PendingCron[] }) {
  const agentDisplayFormat = useMonitorStore((s) => s.settings.agentDisplayFormat)
  const runs = useMonitorStore((s) => s.data?.runs)
  const { t } = useI18n()

  const nameMap = useMemo(() => {
    const m = new Map<string, string>()
    if (runs) {
      for (const r of runs) {
        if (r.tool === 'openClaw' && r.agentName && r.agentDisplayName) {
          m.set(r.agentName, r.agentDisplayName)
        }
      }
    }
    return m
  }, [runs])

  const sorted = useMemo(() => expandCrons(crons), [crons])
  if (sorted.length === 0) return null
  return (
    <div className="cron-list">
      <div className="cron-list-header">{t('ui.scheduled')}</div>
      {sorted.map((cron) => {
        const agent = formatCronAgent(cron, agentDisplayFormat, nameMap)
        return (
          <div key={cron.id} className="cron-row">
            <span className="cron-schedule">{formatScheduleHuman(cron.scheduleHuman)}</span>
            <span className="cron-name">{cron.name}</span>
            {agent && <span className="cron-agent">{agent}</span>}
          </div>
        )
      })}
    </div>
  )
}

function SourceColumn({
  tool,
  runs,
  crons,
  showEmptyState = true,
}: {
  tool: ToolKind
  runs: RunRecord[]
  crons?: PendingCron[]
  showEmptyState?: boolean
}) {
  const { t } = useI18n()
  const selectRun = useMonitorStore((s) => s.selectRun)
  const focusedRunId = useMonitorStore((s) => s.focusedRunId)
  const health = useMonitorStore((s) => s.data?.adapterHealth.find((h) => h.tool === tool))
  const identity = useMonitorStore((s) => s.data?.identities.find((id) => id.tool === tool))
  const agentDisplayFormat = useMonitorStore((s) => s.settings.agentDisplayFormat)
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)

  const counts = useMemo(() => {
    const { active, waiting, done } = summarizeRunsByState(runs)
    return { running: active, waiting, done }
  }, [runs])

  const grouped = useMemo(() => groupRunsByProject(runs, agentDisplayFormat), [runs, agentDisplayFormat])

  const authMode = identity?.authMode ?? runs[0]?.authMode
  const isApiKey = authMode === 'api_key'

  const quota = useMemo(() => {
    const activeRun = runs.find((r) => r.quota.fiveHourUsedPct != null || r.quota.sevenDayUsedPct != null)
    return activeRun?.quota
  }, [runs])

  const hasQuotaConcept = tool !== 'openClaw'
  const subscriptionHasRuns = hasQuotaConcept && !isApiKey && runs.length > 0
  const quotaLoading = subscriptionHasRuns && !quota

  const healthTitle = health
    ? `${health.mode} | ${health.online ? 'online' : 'offline'} | ${health.freshness}${health.lastError ? ` | ${health.lastError}` : ''}`
    : undefined

  return (
    <div className={`source-column ${sourceAccents[tool]}`}>
      <div className="source-header">
        <div className="source-top-row">
          <div className="source-name-row">
            <h3 className="source-name">{sourceLabels[tool]}</h3>
            <span
              className={`source-dot ${health?.online ? 'online' : 'offline'}`}
              title={healthTitle}
            />
          </div>
          {isApiKey ? (
            <PayAsYouGoBadge />
          ) : quotaLoading ? (
            <div className="quota-bars">
              <QuotaBar label="5hrs" pct={undefined} loading />
              <QuotaBar label="7days" pct={undefined} loading />
            </div>
          ) : quota ? (
            <div className="quota-bars">
              <QuotaBar label="5hrs" pct={quota.fiveHourUsedPct} />
              <QuotaBar label="7days" pct={quota.sevenDayUsedPct} />
            </div>
          ) : null}
        </div>
        <div className="source-meta">
          <span className={`source-count${counts.running > 0 ? ' has-running' : ''}`}>
            <span className="dot-running" /> {counts.running} {t('stateCount.running').toLowerCase()}
          </span>
          <span className={`source-count${counts.waiting > 0 ? ' has-waiting' : ''}`}>
            <span className="dot-waiting" /> {counts.waiting} {t('stateCount.waiting').toLowerCase()}
          </span>
          <span className="source-count">
            <span className="dot-done" /> {counts.done} {t('stateCount.done').toLowerCase()}
          </span>
        </div>
      </div>
      <div className="session-list">
        {showEmptyState && runs.length === 0 && (
          <div className="empty-state">{t('monitor.noSessions')}</div>
        )}
        {grouped.active.map((g) => (
          <ProjectGroupSection key={`a-${g.key}`} group={g} selectRun={selectRun} focusedRunId={focusedRunId} />
        ))}
        {grouped.waiting.map((g) => (
          <ProjectGroupSection key={`w-${g.key}`} group={g} selectRun={selectRun} focusedRunId={focusedRunId} />
        ))}
        {grouped.done.length > 0 && (
          <div className="done-divider">
            <span className="done-divider-label">
              {t('monitor.doneIn').replace('{period}', periodDisplayLabels[monitorPeriod] ?? monitorPeriod)}
            </span>
          </div>
        )}
        {grouped.done.map((g) => (
          <ProjectGroupSection key={`d-${g.key}`} group={g} selectRun={selectRun} focusedRunId={focusedRunId} />
        ))}
      </div>
      {crons && crons.length > 0 && <CronList crons={crons} />}
    </div>
  )
}

function MobileSourceTabs({
  selected,
  onSelect,
  counts,
  visibleTools,
}: {
  selected: ToolKind
  onSelect: (t: ToolKind) => void
  counts: Record<ToolKind, number>
  visibleTools: ToolKind[]
}) {
  return (
    <div className="mobile-source-tabs">
      {visibleTools.map((tool) => (
        <button
          key={tool}
          className={`mobile-source-tab ${sourceAccents[tool]} ${selected === tool ? 'active' : ''}`}
          onClick={() => onSelect(tool)}
        >
          {sourceLabels[tool]} ({counts[tool]})
        </button>
      ))}
    </div>
  )
}

const periodDisplayLabels: Record<string, string> = {
  '30m': '30 min', '1h': '1 hour', '2h': '2 hours',
  '4h': '4 hours', '8h': '8 hours', '24h': '24 hours',
}

export function MonitorView() {
  const data = useMonitorStore((s) => s.data)
  const connectionStatus = useMonitorStore((s) => s.connectionStatus)
  const columnLayout = useMonitorStore((s) => s.settings.columnLayout)
  const monitorPeriod = useMonitorStore((s) => s.settings.monitorPeriod)
  const panelConfig = useMonitorStore((s) => s.settings.panelConfig)
  const filterRules = useMonitorStore((s) => s.settings.filterRules)
  const [mobileSource, setMobileSource] = useState<ToolKind>('claude')
  const { t } = useI18n()
  const isNarrowLayout = useMediaQuery(NARROW_LAYOUT_QUERY)

  const visiblePanels = useMemo(
    () => buildVisiblePanels(panelConfig),
    [panelConfig],
  )

  const sessionsBySource = useMemo(() => {
    if (!data) {
      return { claude: [], codex: [], openClaw: [], hermes: [] } satisfies Record<ToolKind, RunRecord[]>
    }
    return buildVisibleRunsBySource(data.runs, filterRules, monitorPeriod)
  }, [data, monitorPeriod, filterRules])

  const hasVisibleRuns = visiblePanels.some((tool) => sessionsBySource[tool].length > 0)

  const adaptiveStyle = useMemo(() => {
    if (columnLayout !== 'adaptive') return undefined
    const cols = visiblePanels
      .map((tool) => `${Math.max(sessionsBySource[tool].length, 1)}fr`)
      .join(' ')
    return { gridTemplateColumns: cols }
  }, [columnLayout, sessionsBySource, visiblePanels])

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

  const sourceCounts: Record<ToolKind, number> = {
    claude: sessionsBySource.claude.length,
    codex: sessionsBySource.codex.length,
    openClaw: sessionsBySource.openClaw.length,
    hermes: sessionsBySource.hermes.length,
  }

  const effectiveMobileSource = visiblePanels.includes(mobileSource)
    ? mobileSource
    : visiblePanels[0] ?? 'claude'

  return (
    <div className="monitor-view">
      {connectionStatus === 'offline' && (
        <div className="status-notice offline">
          <strong>{t('monitor.offlineTitle')}</strong>
          <span>{t('monitor.offlineHint')}</span>
        </div>
      )}
      <AttentionBanner items={data.attentions} />
      {!hasVisibleRuns && (
        <div className="empty-state-panel">
          <strong>{t('monitor.emptyTitle')}</strong>
          <span>{t('monitor.emptyHint')}</span>
        </div>
      )}
      <section className="monitor-board-panel">
        {isNarrowLayout ? (
          <>
            <MobileSourceTabs
              selected={effectiveMobileSource}
              onSelect={setMobileSource}
              counts={sourceCounts}
              visibleTools={visiblePanels}
            />
            <div className="source-columns-mobile">
              <SourceColumn
                tool={effectiveMobileSource}
                runs={sessionsBySource[effectiveMobileSource]}
                crons={effectiveMobileSource === 'openClaw' || effectiveMobileSource === 'hermes' ? data.pendingCrons : undefined}
                showEmptyState={hasVisibleRuns}
              />
            </div>
          </>
        ) : (
          <div
            className={`source-columns source-columns-desktop ${columnLayout === 'adaptive' ? 'adaptive' : ''}`}
            style={adaptiveStyle}
          >
            {visiblePanels.map((tool) => (
              <SourceColumn
                key={tool}
                tool={tool}
                runs={sessionsBySource[tool]}
                crons={tool === 'openClaw' || tool === 'hermes' ? data.pendingCrons : undefined}
                showEmptyState={hasVisibleRuns}
              />
            ))}
          </div>
        )}
      </section>
    </div>
  )
}

import { allTools } from './constants'
import { getGroupKey } from './format'
import type { AgentDisplayFormat, FilterRules, MonitorPeriod, PanelEntry, ToolFilter } from './preferences'
import type { RunRecord, ToolKind } from './types'

export interface MonitorRunCounts {
  active: number
  waiting: number
  done: number
}

export const periodToMs: Record<MonitorPeriod, number> = {
  '30m': 30 * 60_000,
  '1h': 60 * 60_000,
  '2h': 2 * 60 * 60_000,
  '4h': 4 * 60 * 60_000,
  '8h': 8 * 60 * 60_000,
  '24h': 24 * 60 * 60_000,
}

/** Full-word labels for the monitor period presets (e.g. "1 hour"). */
export const monitorPeriodLongLabels: Record<MonitorPeriod, string> = {
  '30m': '30 min',
  '1h': '1 hour',
  '2h': '2 hours',
  '4h': '4 hours',
  '8h': '8 hours',
  '24h': '24 hours',
}

function sortRunsForMonitor(runs: RunRecord[]): RunRecord[] {
  const order: Record<string, number> = { active: 0, waitingApproval: 1 }
  return [...runs].sort((a, b) => {
    const stateDelta = (order[a.state] ?? 2) - (order[b.state] ?? 2)
    if (stateDelta !== 0) return stateDelta
    return new Date(b.lastActivityAt).getTime() - new Date(a.lastActivityAt).getTime()
  })
}

export function buildVisiblePanels(panelConfig: PanelEntry[]): ToolKind[] {
  return panelConfig.filter((entry) => entry.enabled).map((entry) => entry.tool)
}

export function isRunVisibleInMonitorPeriod(
  run: RunRecord,
  monitorPeriod: MonitorPeriod,
  now = Date.now(),
): boolean {
  if (run.state === 'active' || run.state === 'waitingApproval') return true

  const activityTime = new Date(run.lastActivityAt).getTime()
  if (!Number.isFinite(activityTime)) return false

  const cutoff = now - (periodToMs[monitorPeriod] ?? periodToMs['1h'])
  return activityTime >= cutoff
}

export function summarizeRunsByState(runs: RunRecord[]): MonitorRunCounts {
  return runs.reduce<MonitorRunCounts>((counts, run) => {
    if (run.state === 'active') {
      counts.active += 1
    } else if (run.state === 'waitingApproval') {
      counts.waiting += 1
    } else {
      counts.done += 1
    }
    return counts
  }, { active: 0, waiting: 0, done: 0 })
}

export function buildMonitorStateStats(
  runs: RunRecord[],
  monitorPeriod: MonitorPeriod,
  now = Date.now(),
): MonitorRunCounts {
  return summarizeRunsByState(runs.filter((run) => isRunVisibleInMonitorPeriod(run, monitorPeriod, now)))
}

export function matchesToolFilter(run: RunRecord, filter: ToolFilter): boolean {
  if (filter.mode === 'off' || filter.patterns.length === 0) return true

  const key = run.tool === 'openClaw'
    ? (run.agentName ?? '')
    : (run.workspacePath || run.projectName || '')
  const keyLower = key.toLowerCase()
  const hit = filter.patterns.some((pattern) => keyLower.includes(pattern.toLowerCase()))
  return filter.mode === 'include' ? hit : !hit
}

export function buildVisibleRunsBySource(
  runs: RunRecord[],
  filterRules: FilterRules,
  monitorPeriod: MonitorPeriod,
): Record<ToolKind, RunRecord[]> {
  const sessionsBySource = Object.fromEntries(
    allTools.map((tool) => [tool, [] as RunRecord[]]),
  ) as Record<ToolKind, RunRecord[]>

  for (const run of runs) {
    if (!sessionsBySource[run.tool]) continue

    if (!isRunVisibleInMonitorPeriod(run, monitorPeriod)) continue

    if (!matchesToolFilter(run, filterRules[run.tool])) continue
    sessionsBySource[run.tool].push(run)
  }

  for (const tool of allTools) {
    sessionsBySource[tool] = sortRunsForMonitor(sessionsBySource[tool])
  }

  return sessionsBySource
}

export function buildVisibleRunIds(
  sessionsBySource: Record<ToolKind, RunRecord[]>,
  visiblePanels: ToolKind[],
  agentDisplayFormat: AgentDisplayFormat,
): string[] {
  const runIds: string[] = []

  for (const tool of visiblePanels) {
    const buckets = new Map<string, string[]>()
    for (const run of sessionsBySource[tool]) {
      const key = getGroupKey(run, agentDisplayFormat)
      const existing = buckets.get(key)
      if (existing) {
        existing.push(run.id)
      } else {
        buckets.set(key, [run.id])
      }
    }

    for (const ids of buckets.values()) {
      runIds.push(...ids)
    }
  }

  return runIds
}

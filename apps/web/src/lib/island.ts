import { allTools } from './constants'
import type { RunRecord, ToolKind } from './types'

export type IslandPriority = 'waiting' | 'active' | 'freshDone' | 'done'

export type IslandItem = {
  id: string
  run: RunRecord
  priority: IslandPriority
  unread: boolean
}

export type IslandCounts = {
  active: number
  waiting: number
  unreadDone: number
}

const doneStates = new Set(['completed', 'idle', 'stale', 'cancelled'])
const priorityRank: Record<IslandPriority, number> = {
  waiting: 0,
  active: 1,
  freshDone: 2,
  done: 3,
}

const toolRank = Object.fromEntries(allTools.map((tool, idx) => [tool, idx])) as Record<ToolKind, number>

function isWaiting(run: RunRecord): boolean {
  return run.state === 'waitingApproval' || run.pendingApproval
}

function isDone(run: RunRecord): boolean {
  return doneStates.has(run.state)
}

function timestampValue(iso: string): number {
  const value = Date.parse(iso)
  return Number.isFinite(value) ? value : 0
}

function toIslandItem(run: RunRecord, visitedRunIds: ReadonlySet<string>): IslandItem | null {
  if (isWaiting(run)) return { id: run.id, run, priority: 'waiting', unread: false }
  if (run.state === 'active') return { id: run.id, run, priority: 'active', unread: false }
  if (!isDone(run)) return null
  const unread = !visitedRunIds.has(run.id)
  return { id: run.id, run, priority: unread ? 'freshDone' : 'done', unread }
}

export function buildIslandCounts(
  runs: RunRecord[],
  visitedRunIds: ReadonlySet<string>,
): IslandCounts {
  let active = 0
  let waiting = 0
  let unreadDone = 0
  for (const run of runs) {
    if (isWaiting(run)) {
      waiting++
    } else if (run.state === 'active') {
      active++
    } else if (isDone(run) && !visitedRunIds.has(run.id)) {
      unreadDone++
    }
  }
  return { active, waiting, unreadDone }
}

export function buildIslandItems(
  runs: RunRecord[],
  visitedRunIds: ReadonlySet<string>,
  limit = 8,
): IslandItem[] {
  return runs
    .map((run) => toIslandItem(run, visitedRunIds))
    .filter((item): item is IslandItem => item !== null)
    .sort((a, b) => {
      const priorityDelta = priorityRank[a.priority] - priorityRank[b.priority]
      if (priorityDelta !== 0) return priorityDelta
      const activityDelta = timestampValue(b.run.lastActivityAt) - timestampValue(a.run.lastActivityAt)
      if (activityDelta !== 0) return activityDelta
      return toolRank[a.run.tool] - toolRank[b.run.tool]
    })
    .slice(0, Math.max(0, limit))
}

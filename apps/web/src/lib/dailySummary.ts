import { parseMs } from './dateRange'
import type { CommitRecord, RunRecord, ToolKind, UsageBucket } from './types'
import {
  buildUsageBucketIndex,
  collectRunUsageSlicesFromIndex,
  sumUsageSlices,
  type UsageSlice,
} from './usage'

export interface SourceBreakdown {
  source: ToolKind
  sessions: number
  tokens: number
  cost: number
}

export interface ProjectBreakdown {
  project: string
  sessions: number
  tokens: number
  cost: number
}

export interface DailySummaryData {
  date: Date
  sessions: number
  inputs: number
  inputTokens: number
  outputTokens: number
  totalTokens: number
  cost: number
  commits: number
  durationMs: number
  bySource: SourceBreakdown[]
  byProject: ProjectBreakdown[]
  commitList: CommitRecord[]
}

/**
 * Compute the day range for a given date and day boundary hour.
 * dayStartHour=0 → midnight to midnight (default).
 * dayStartHour=6 → 06:00 to 06:00 next day.
 * The "date" anchors the range: for dayStartHour=6 and date=Apr 3,
 * the range is Apr 3 06:00 → Apr 4 06:00.
 */
export function dayRange(date: Date, dayStartHour = 0): { startMs: number; endMs: number } {
  const start = new Date(date)
  start.setHours(dayStartHour, 0, 0, 0)
  const end = new Date(start)
  end.setDate(end.getDate() + 1)
  return { startMs: start.getTime(), endMs: end.getTime() }
}

export function buildDailySummary(
  date: Date,
  runs: RunRecord[],
  usageBuckets: UsageBucket[],
  commits: CommitRecord[],
  dayStartHour = 0,
): DailySummaryData {
  const { startMs, endMs } = dayRange(date, dayStartHour)
  const bucketIndex = buildUsageBucketIndex(usageBuckets)
  const slices = collectRunUsageSlicesFromIndex(runs, bucketIndex, startMs, endMs)
  const totals = sumUsageSlices(slices.map((s) => s.usage))

  let durationMs = 0
  for (const { run } of slices) {
    const s = parseMs(run.startedAt)
    const e = parseMs(run.lastActivityAt)
    if (s == null || e == null || e <= s) continue
    const clampedStart = Math.max(s, startMs)
    const clampedEnd = Math.min(e, endMs)
    durationMs += Math.max(clampedEnd - clampedStart, 0)
  }

  const sourceMap = new Map<ToolKind, { sessions: number; usage: UsageSlice[] }>()
  const projectMap = new Map<string, { sessions: number; usage: UsageSlice[] }>()

  for (const { run, usage } of slices) {
    const src = sourceMap.get(run.tool) ?? { sessions: 0, usage: [] }
    src.sessions++
    src.usage.push(usage)
    sourceMap.set(run.tool, src)

    const proj = run.projectName || run.workspaceShort
    const p = projectMap.get(proj) ?? { sessions: 0, usage: [] }
    p.sessions++
    p.usage.push(usage)
    projectMap.set(proj, p)
  }

  const bySource: SourceBreakdown[] = [...sourceMap.entries()]
    .map(([source, data]) => {
      const s = sumUsageSlices(data.usage)
      return { source, sessions: data.sessions, tokens: s.totalTokens, cost: s.costUsd ?? 0 }
    })
    .sort((a, b) => b.tokens - a.tokens)

  const byProject: ProjectBreakdown[] = [...projectMap.entries()]
    .map(([project, data]) => {
      const s = sumUsageSlices(data.usage)
      return { project, sessions: data.sessions, tokens: s.totalTokens, cost: s.costUsd ?? 0 }
    })
    .sort((a, b) => b.cost - a.cost)

  const dayCommits = commits.filter((c) => {
    const ms = parseMs(c.committedAt) ?? 0
    return ms >= startMs && ms < endMs
  })

  return {
    date,
    sessions: slices.length,
    inputs: totals.messageCount,
    inputTokens: totals.inputTokens,
    outputTokens: totals.outputTokens,
    totalTokens: totals.totalTokens,
    cost: totals.costUsd ?? 0,
    commits: dayCommits.length,
    durationMs,
    bySource,
    byProject,
    commitList: dayCommits,
  }
}

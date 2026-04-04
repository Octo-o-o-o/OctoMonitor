import { sourceLabels } from './constants'
import { parseMs } from './dateRange'
import { formatDuration, formatTokens } from './format'
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
    const s = parseMs(run.startedAt) ?? 0
    const e = parseMs(run.lastActivityAt) ?? 0
    if (s > 0 && e > s) {
      const clampedStart = Math.max(s, startMs)
      const clampedEnd = Math.min(e, endMs)
      durationMs += Math.max(clampedEnd - clampedStart, 0)
    }
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

export function buildBasicReport(data: DailySummaryData, locale: 'en' | 'zh'): string {
  const dateStr = data.date.toLocaleDateString(locale === 'zh' ? 'zh-CN' : 'en-US', {
    weekday: 'long',
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  })

  const lines: string[] = []

  if (locale === 'zh') {
    lines.push(`## ${dateStr} \u5DE5\u4F5C\u65E5\u62A5`)
    lines.push('')
    lines.push(`\u4F1A\u8BDD ${data.sessions} \u6B21 | \u8F93\u5165 ${data.inputs} \u6B21 | Tokens ${formatTokens(data.totalTokens)} | \u8D39\u7528 $${data.cost.toFixed(2)} | Commits ${data.commits} | \u65F6\u957F ${formatDuration(data.durationMs)}`)
  } else {
    lines.push(`## Daily Report — ${dateStr}`)
    lines.push('')
    lines.push(`${data.sessions} sessions | ${data.inputs} inputs | ${formatTokens(data.totalTokens)} tokens | $${data.cost.toFixed(2)} | ${data.commits} commits | ${formatDuration(data.durationMs)}`)
  }

  if (data.bySource.length > 0) {
    lines.push('')
    lines.push(locale === 'zh' ? '### \u6309\u6765\u6E90' : '### By Source')
    for (const s of data.bySource) {
      lines.push(`- **${sourceLabels[s.source] ?? s.source}**: ${s.sessions} sessions, ${formatTokens(s.tokens)} tokens, $${s.cost.toFixed(2)}`)
    }
  }

  if (data.byProject.length > 0) {
    lines.push('')
    lines.push(locale === 'zh' ? '### \u6309\u9879\u76EE' : '### By Project')
    for (const p of data.byProject) {
      lines.push(`- **${p.project}**: ${p.sessions} sessions, ${formatTokens(p.tokens)} tokens, $${p.cost.toFixed(2)}`)
    }
  }

  if (data.commitList.length > 0) {
    lines.push('')
    lines.push('### Commits')
    for (const c of data.commitList.slice(0, 10)) {
      lines.push(`- \`${c.shortSha}\` ${c.summary} (${c.repoName})`)
    }
    if (data.commitList.length > 10) {
      lines.push(locale === 'zh' ? `\u2026\u8FD8\u6709 ${data.commitList.length - 10} \u6761` : `…and ${data.commitList.length - 10} more`)
    }
  }

  return lines.join('\n')
}

export function buildAiPrompt(data: DailySummaryData): string {
  const lines: string[] = []
  lines.push('Generate a concise daily work report (3-5 sentences) for an AI coding tool user.')
  lines.push('')
  lines.push(`Date: ${data.date.toISOString().slice(0, 10)}`)
  lines.push(`Sessions: ${data.sessions}`)
  lines.push(`Inputs: ${data.inputs}`)
  lines.push(`Tokens: ${formatTokens(data.totalTokens)} (in: ${formatTokens(data.inputTokens)}, out: ${formatTokens(data.outputTokens)})`)
  lines.push(`Cost: $${data.cost.toFixed(2)}`)
  lines.push(`Commits: ${data.commits}`)

  if (data.bySource.length > 0) {
    lines.push('')
    lines.push('By source:')
    for (const s of data.bySource) {
      lines.push(`- ${sourceLabels[s.source] ?? s.source}: ${s.sessions} sessions, ${formatTokens(s.tokens)} tokens`)
    }
  }

  if (data.byProject.length > 0) {
    lines.push('')
    lines.push('By project:')
    for (const p of data.byProject) {
      lines.push(`- ${p.project}: ${p.sessions} sessions, $${p.cost.toFixed(2)}`)
    }
  }

  if (data.commitList.length > 0) {
    lines.push('')
    lines.push('Commits:')
    for (const c of data.commitList.slice(0, 15)) {
      lines.push(`- ${c.summary} (${c.repoName})`)
    }
  }

  lines.push('')
  lines.push('Write a brief, professional summary highlighting what was accomplished, which projects got the most attention, and any notable patterns. Output only the summary text, no markdown headers.')

  return lines.join('\n')
}

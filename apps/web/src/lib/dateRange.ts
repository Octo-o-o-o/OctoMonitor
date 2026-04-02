import type { CommitRecord, RunRecord, UsageBucket } from './types'

function parseMs(value: string): number | undefined {
  const ms = Date.parse(value)
  return Number.isFinite(ms) ? ms : undefined
}

export function startOfDay(d: Date): Date {
  const r = new Date(d)
  r.setHours(0, 0, 0, 0)
  return r
}

export function endOfDay(d: Date): Date {
  const r = new Date(d)
  r.setHours(23, 59, 59, 999)
  return r
}

export function buildDateRangeFromMs(minMs: number, maxMs: number): { from: Date; to: Date } | null {
  if (!Number.isFinite(minMs) || !Number.isFinite(maxMs)) {
    return null
  }

  return {
    from: startOfDay(new Date(minMs)),
    to: endOfDay(new Date(maxMs)),
  }
}

export function buildUsageDateRange(
  runs: RunRecord[],
  buckets: UsageBucket[],
): { from: Date; to: Date } | null {
  let minMs = Number.POSITIVE_INFINITY
  let maxMs = Number.NEGATIVE_INFINITY

  for (const bucket of buckets) {
    const startMs = parseMs(bucket.start)
    const endMs = parseMs(bucket.end)
    if (startMs != null) minMs = Math.min(minMs, startMs)
    if (endMs != null) maxMs = Math.max(maxMs, endMs)
  }

  if (!Number.isFinite(minMs) || !Number.isFinite(maxMs)) {
    for (const run of runs) {
      const startMs = parseMs(run.startedAt)
      const endMs = parseMs(run.lastActivityAt)
      if (startMs != null) minMs = Math.min(minMs, startMs)
      if (endMs != null) maxMs = Math.max(maxMs, endMs)
    }
  }

  return buildDateRangeFromMs(minMs, maxMs)
}

export function buildCommitDateRange(commits: CommitRecord[]): { from: Date; to: Date } | null {
  let minMs = Number.POSITIVE_INFINITY
  let maxMs = Number.NEGATIVE_INFINITY

  for (const commit of commits) {
    const committedAtMs = parseMs(commit.committedAt)
    if (committedAtMs == null) continue
    minMs = Math.min(minMs, committedAtMs)
    maxMs = Math.max(maxMs, committedAtMs)
  }

  return buildDateRangeFromMs(minMs, maxMs)
}

import type { RunRecord, UsageBucket } from './types'

type UsageSliceSource = {
  start: string
  end: string
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheWriteTokens: number
  totalTokens: number
  costUsd?: number | null
  messageCount?: number
}

export interface UsageSlice {
  inputTokens: number
  outputTokens: number
  cacheReadTokens: number
  cacheWriteTokens: number
  totalTokens: number
  costUsd?: number | null
  messageCount: number
}

export interface RunUsageSlice {
  run: RunRecord
  usage: UsageSlice
}

export type UsageBucketIndex = Map<string, UsageBucket>

const emptySlice: UsageSlice = {
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheWriteTokens: 0,
  totalTokens: 0,
  costUsd: 0,
  messageCount: 0,
}

function parseMs(value: string): number | undefined {
  const ms = Date.parse(value)
  return Number.isFinite(ms) ? ms : undefined
}

function scaleUsage(source: UsageSliceSource, ratio: number): UsageSlice {
  return {
    inputTokens: source.inputTokens * ratio,
    outputTokens: source.outputTokens * ratio,
    cacheReadTokens: source.cacheReadTokens * ratio,
    cacheWriteTokens: source.cacheWriteTokens * ratio,
    totalTokens: source.totalTokens * ratio,
    costUsd: source.costUsd == null ? undefined : source.costUsd * ratio,
    messageCount: (source.messageCount ?? 0) * ratio,
  }
}

export function intervalOverlapMs(
  startMs: number,
  endMs: number,
  rangeStartMs: number,
  rangeEndMs: number,
): number {
  const normalizedEnd = Math.max(endMs, startMs)
  const overlapStart = Math.max(startMs, rangeStartMs)
  const overlapEnd = Math.min(normalizedEnd, rangeEndMs)
  return Math.max(overlapEnd - overlapStart, 0)
}

export function sliceUsageBucket(
  bucket: UsageSliceSource,
  rangeStartMs: number,
  rangeEndMs: number,
): UsageSlice {
  if (rangeEndMs <= rangeStartMs) {
    return emptySlice
  }

  const startMs = parseMs(bucket.start)
  const endMs = parseMs(bucket.end)
  if (startMs == null || endMs == null) {
    return emptySlice
  }

  const durationMs = Math.max(endMs - startMs, 0)
  if (durationMs === 0) {
    return startMs >= rangeStartMs && startMs <= rangeEndMs
      ? { ...bucket, messageCount: bucket.messageCount ?? 0 }
      : emptySlice
  }

  const overlapMs = intervalOverlapMs(startMs, endMs, rangeStartMs, rangeEndMs)
  if (overlapMs <= 0) {
    return emptySlice
  }

  return scaleUsage(bucket, overlapMs / durationMs)
}

export function hasUsage(slice: UsageSlice): boolean {
  return slice.totalTokens > 0 || (slice.costUsd ?? 0) > 0
}

export function runOverlapMs(
  run: RunRecord,
  rangeStartMs: number,
  rangeEndMs: number,
): number {
  const startMs = parseMs(run.startedAt)
  const endMs = parseMs(run.lastActivityAt)
  if (startMs == null || endMs == null) {
    return 0
  }
  return intervalOverlapMs(startMs, endMs, rangeStartMs, rangeEndMs)
}

export function runOverlapsRange(
  run: RunRecord,
  rangeStartMs: number,
  rangeEndMs: number,
): boolean {
  return runOverlapMs(run, rangeStartMs, rangeEndMs) > 0
}

export function sliceRunUsage(
  run: RunRecord,
  bucket: UsageBucket | undefined,
  rangeStartMs: number,
  rangeEndMs: number,
): UsageSlice {
  return sliceUsageBucket(
    {
      start: bucket?.start ?? run.startedAt,
      end: bucket?.end ?? run.lastActivityAt,
      inputTokens: bucket?.inputTokens ?? run.tokens.input,
      outputTokens: bucket?.outputTokens ?? run.tokens.output,
      cacheReadTokens: bucket?.cacheReadTokens ?? run.tokens.cacheRead,
      cacheWriteTokens: bucket?.cacheWriteTokens ?? run.tokens.cacheWrite,
      totalTokens: bucket?.totalTokens ?? run.tokens.total,
      costUsd: bucket?.costUsd ?? run.cost.usd,
      messageCount: run.messageCount,
    },
    rangeStartMs,
    rangeEndMs,
  )
}

export function buildUsageBucketIndex(buckets: UsageBucket[]): UsageBucketIndex {
  const map: UsageBucketIndex = new Map()
  for (const bucket of buckets) {
    const runId = bucket.scope.runId
    if (runId != null && runId !== '') {
      map.set(runId, bucket)
    }
  }
  return map
}

export function collectRunUsageSlicesFromIndex(
  runs: RunRecord[],
  bucketByRunId: UsageBucketIndex,
  rangeStartMs: number,
  rangeEndMs: number,
): RunUsageSlice[] {
  const out: RunUsageSlice[] = []

  for (const run of runs) {
    const usage = sliceRunUsage(run, bucketByRunId.get(run.id), rangeStartMs, rangeEndMs)
    if (!hasUsage(usage) && !runOverlapsRange(run, rangeStartMs, rangeEndMs)) {
      continue
    }
    out.push({ run, usage })
  }

  return out
}

export function collectRunUsageSlices(
  runs: RunRecord[],
  buckets: UsageBucket[],
  rangeStartMs: number,
  rangeEndMs: number,
): RunUsageSlice[] {
  return collectRunUsageSlicesFromIndex(
    runs,
    buildUsageBucketIndex(buckets),
    rangeStartMs,
    rangeEndMs,
  )
}

export function sumUsageSlices(slices: Iterable<UsageSlice>): UsageSlice {
  const total: UsageSlice = { ...emptySlice }

  for (const slice of slices) {
    total.inputTokens += slice.inputTokens
    total.outputTokens += slice.outputTokens
    total.cacheReadTokens += slice.cacheReadTokens
    total.cacheWriteTokens += slice.cacheWriteTokens
    total.totalTokens += slice.totalTokens
    total.costUsd = (total.costUsd ?? 0) + (slice.costUsd ?? 0)
    total.messageCount += slice.messageCount
  }

  return total
}

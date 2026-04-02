import type { RunRecord, UsageBucket } from './types'
import {
  buildUsageBucketIndex,
  collectRunUsageSlices,
  runOverlapMs,
  sliceRunUsage,
  sliceUsageBucket,
  sumUsageSlices,
} from './usage'

function createRun(overrides: Partial<RunRecord> = {}): RunRecord {
  return {
    id: 'run-1',
    tool: 'codex',
    sourceMode: 'test',
    projectName: 'OctoMonitor',
    workspacePath: '/tmp/octomonitor',
    workspaceShort: '~/octomonitor',
    model: 'gpt-5-codex',
    provider: 'openai',
    agentName: null,
    agentDisplayName: null,
    accountAlias: null,
    authMode: null,
    authVerified: true,
    sessionId: null,
    threadId: null,
    sessionKey: null,
    transcriptPath: null,
    startedAt: '2026-04-01T00:00:00.000Z',
    lastActivityAt: '2026-04-01T01:00:00.000Z',
    elapsedMs: 3_600_000,
    state: 'completed',
    lastAction: null,
    lastTail: null,
    pendingApproval: false,
    firstQuestion: null,
    lastQuestion: null,
    errorMessage: null,
    messageCount: 1,
    tokens: {
      input: 1_000,
      output: 500,
      cacheRead: 100,
      cacheWrite: 0,
      total: 1_600,
      context: 0,
    },
    cost: {
      usd: 1.6,
      confidence: 'estimated',
    },
    quota: {
      fiveHourUsedPct: null,
      sevenDayUsedPct: null,
      resetAt: [],
      confidence: 'derived',
    },
    source: {
      confidence: 'live',
      freshness: 'hot',
      lastUpdatedAt: '2026-04-01T01:00:00.000Z',
    },
    vcs: null,
    originLabel: null,
    originProvider: null,
    ...overrides,
  }
}

function createBucket(overrides: Partial<UsageBucket> = {}): UsageBucket {
  return {
    scope: {
      runId: 'run-1',
      tool: 'codex',
    },
    window: 'session',
    start: '2026-04-01T00:00:00.000Z',
    end: '2026-04-01T01:00:00.000Z',
    inputTokens: 1_000,
    outputTokens: 500,
    cacheReadTokens: 100,
    cacheWriteTokens: 0,
    totalTokens: 1_600,
    costUsd: 1.6,
    confidence: 'estimated',
    ...overrides,
  }
}

describe('usage slicing', () => {
  it('prorates usage buckets by overlap duration', () => {
    const bucket = createBucket()
    const slice = sliceUsageBucket(
      bucket,
      Date.parse('2026-04-01T00:30:00.000Z'),
      Date.parse('2026-04-01T01:00:00.000Z'),
    )

    expect(slice.totalTokens).toBeCloseTo(800)
    expect(slice.inputTokens).toBeCloseTo(500)
    expect(slice.costUsd).toBeCloseTo(0.8)
  })

  it('uses run totals when a matching bucket is unavailable', () => {
    const run = createRun()
    const slice = sliceRunUsage(
      run,
      undefined,
      Date.parse('2026-04-01T00:15:00.000Z'),
      Date.parse('2026-04-01T00:45:00.000Z'),
    )

    expect(slice.totalTokens).toBeCloseTo(800)
    expect(slice.costUsd).toBeCloseTo(0.8)
  })

  it('indexes usage buckets by run id from scope metadata', () => {
    const bucket = createBucket()
    const index = buildUsageBucketIndex([bucket])

    expect(index.get('run-1')).toEqual(bucket)
  })

  it('collects run usage slices with the shared run-to-bucket mapping', () => {
    const slices = collectRunUsageSlices(
      [createRun()],
      [createBucket()],
      Date.parse('2026-04-01T00:30:00.000Z'),
      Date.parse('2026-04-01T01:00:00.000Z'),
    )

    expect(slices).toHaveLength(1)
    expect(slices[0]?.usage.totalTokens).toBeCloseTo(800)
    expect(slices[0]?.usage.costUsd).toBeCloseTo(0.8)
  })

  it('sums shared usage slices into one token/cost total', () => {
    const total = sumUsageSlices([
      {
        inputTokens: 100,
        outputTokens: 50,
        cacheReadTokens: 10,
        cacheWriteTokens: 0,
        totalTokens: 160,
        costUsd: 0.16,
      },
      {
        inputTokens: 200,
        outputTokens: 25,
        cacheReadTokens: 5,
        cacheWriteTokens: 20,
        totalTokens: 250,
        costUsd: 0.25,
      },
    ])

    expect(total.inputTokens).toBe(300)
    expect(total.outputTokens).toBe(75)
    expect(total.cacheReadTokens).toBe(15)
    expect(total.cacheWriteTokens).toBe(20)
    expect(total.totalTokens).toBe(410)
    expect(total.costUsd).toBeCloseTo(0.41)
  })

  it('measures run overlap duration for shared window filtering', () => {
    const overlapMs = runOverlapMs(
      createRun(),
      Date.parse('2026-04-01T00:15:00.000Z'),
      Date.parse('2026-04-01T00:45:00.000Z'),
    )

    expect(overlapMs).toBe(30 * 60 * 1000)
  })
})

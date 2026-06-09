import { describe, expect, it } from 'vitest'
import { buildDailySummary, dayRange } from './dailySummary'
import type { CommitRecord, RunRecord, UsageBucket } from './types'

function runFixture(overrides: Partial<RunRecord>): RunRecord {
  return {
    id: 'run',
    tool: 'claude',
    sourceMode: 'live',
    projectName: 'Octo',
    workspacePath: '/tmp/octo',
    workspaceShort: '~/octo',
    model: null,
    provider: null,
    agentName: null,
    agentDisplayName: null,
    accountAlias: null,
    authMode: null,
    authVerified: true,
    sessionId: null,
    threadId: null,
    sessionKey: null,
    transcriptPath: null,
    startedAt: '2026-04-15T20:00:00.000Z',
    lastActivityAt: '2026-04-15T21:00:00.000Z',
    elapsedMs: 3_600_000,
    state: 'completed',
    lastAction: null,
    lastTail: null,
    pendingApproval: false,
    firstQuestion: null,
    lastQuestion: null,
    errorMessage: null,
    messageCount: 0,
    tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0, context: 0 },
    cost: { usd: null, confidence: 'derived' },
    quota: { fiveHourUsedPct: null, sevenDayUsedPct: null, resetAt: [], confidence: 'derived' },
    source: {
      confidence: 'derived',
      freshness: 'warm',
      lastUpdatedAt: '2026-04-15T21:00:00.000Z',
    },
    vcs: null,
    originLabel: null,
    originProvider: null,
    ...overrides,
  }
}

function bucketFixture(overrides: Partial<UsageBucket> & { run: RunRecord; tokens: number; cost?: number }): UsageBucket {
  // Scoped to a single run via the `runId` scope so the bucket index can
  // match it back during slice collection.
  const tokens = overrides.tokens
  return {
    scope: { runId: overrides.run.id },
    window: 'hour',
    start: overrides.start ?? overrides.run.startedAt,
    end: overrides.end ?? overrides.run.lastActivityAt,
    inputTokens: tokens * 0.6,
    outputTokens: tokens * 0.4,
    cacheReadTokens: 0,
    cacheWriteTokens: 0,
    totalTokens: tokens,
    costUsd: overrides.cost ?? 0,
    confidence: 'derived',
  }
}

function commitFixture(overrides: Partial<CommitRecord> & { committedAt: string }): CommitRecord {
  const { committedAt, ...rest } = overrides
  return {
    id: overrides.id ?? `c-${committedAt}`,
    repoId: 'repo',
    repoName: 'Octo',
    repoRoot: '/tmp/octo',
    worktreeId: null,
    worktreeName: null,
    sha: 'a'.repeat(40),
    shortSha: 'aaaaaaa',
    authorName: 'Tester',
    committedAt,
    summary: 'a commit',
    filesChanged: 1,
    insertions: 1,
    deletions: 0,
    attributedTokens: 0,
    attributedCostUsd: 0,
    runCount: 0,
    sourceCount: 0,
    confidence: 'derived',
    method: 'readOnlyHeuristic',
    sources: [],
    links: [],
    ...rest,
  }
}

describe('dayRange', () => {
  it('returns a 24h window starting at the local midnight of the given date', () => {
    // Use UTC-anchored ms so the test stays meaningful regardless of test
    // runner timezone — we only assert that the window length matches what
    // the local clock thinks midnight→midnight should be.
    const date = new Date(2026, 3, 15) // April 15 2026 local
    const { startMs, endMs } = dayRange(date)
    const start = new Date(startMs)
    expect(start.getHours()).toBe(0)
    expect(start.getMinutes()).toBe(0)
    expect(start.getSeconds()).toBe(0)
    // Window length is one calendar day in the local timezone; outside of
    // DST transitions this equals 24h. Within DST shift days the length is
    // 23h or 25h, which we explicitly *allow* here — dailySummary callers
    // anchor by date, not by exact duration.
    const lengthHours = (endMs - startMs) / (60 * 60 * 1000)
    expect([23, 24, 25]).toContain(lengthHours)
  })

  it('offsets the window when dayStartHour > 0', () => {
    const date = new Date(2026, 3, 15)
    const { startMs } = dayRange(date, 6)
    expect(new Date(startMs).getHours()).toBe(6)
  })
})

describe('buildDailySummary', () => {
  it('returns zeros when there are no runs and no commits for the day', () => {
    const summary = buildDailySummary(new Date(2026, 3, 15), [], [], [])
    expect(summary.sessions).toBe(0)
    expect(summary.totalTokens).toBe(0)
    expect(summary.cost).toBe(0)
    expect(summary.commits).toBe(0)
    expect(summary.bySource).toEqual([])
    expect(summary.byProject).toEqual([])
    expect(summary.commitList).toEqual([])
  })

  it('aggregates tokens across runs that fall inside the day window', () => {
    // Anchor everything to local-midnight so the day-window math is
    // independent of test runner TZ.
    const date = new Date(2026, 3, 15)
    const start = new Date(date)
    start.setHours(12, 0, 0, 0)
    const end = new Date(start.getTime() + 60 * 60 * 1000)

    const run = runFixture({
      id: 'r1',
      startedAt: start.toISOString(),
      lastActivityAt: end.toISOString(),
    })
    const bucket = bucketFixture({
      run,
      tokens: 1000,
      cost: 2.5,
      start: start.toISOString(),
      end: end.toISOString(),
    })
    const summary = buildDailySummary(date, [run], [bucket], [])
    expect(summary.sessions).toBe(1)
    expect(summary.totalTokens).toBeGreaterThan(0)
    expect(summary.cost).toBeCloseTo(2.5, 1)
  })

  it('counts only commits whose committedAt falls inside the day window', () => {
    const date = new Date(2026, 3, 15)
    const inside = new Date(date)
    inside.setHours(10, 0, 0, 0)
    const yesterday = new Date(date)
    yesterday.setDate(yesterday.getDate() - 1)
    yesterday.setHours(10, 0, 0, 0)

    const commits = [
      commitFixture({ id: 'c-in', committedAt: inside.toISOString() }),
      commitFixture({ id: 'c-out', committedAt: yesterday.toISOString() }),
    ]
    const summary = buildDailySummary(date, [], [], commits)
    expect(summary.commits).toBe(1)
    expect(summary.commitList.map((c) => c.id)).toEqual(['c-in'])
  })

  it('skips runs whose startedAt timestamp cannot be parsed without throwing', () => {
    // dailySummary defends itself against junk timestamps that leak in from
    // historical data so a single malformed run doesn't break the whole day.
    const date = new Date(2026, 3, 15)
    const goodStart = new Date(date)
    goodStart.setHours(12, 0, 0, 0)
    const goodEnd = new Date(goodStart.getTime() + 60 * 60 * 1000)

    const good = runFixture({
      id: 'good',
      startedAt: goodStart.toISOString(),
      lastActivityAt: goodEnd.toISOString(),
    })
    const broken = runFixture({
      id: 'broken',
      startedAt: 'not-a-date',
      lastActivityAt: 'also-not-a-date',
    })
    expect(() =>
      buildDailySummary(date, [good, broken], [], []),
    ).not.toThrow()
  })
})

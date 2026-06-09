import { describe, expect, it } from 'vitest'
import { buildIslandCounts, buildIslandItems } from './island'
import type { RunRecord } from './types'

function runFixture(overrides: Partial<RunRecord>): RunRecord {
  return {
    id: 'run-x',
    tool: 'codex',
    sourceMode: 'live',
    projectName: 'OctoMonitor',
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
    startedAt: '2026-06-09T10:00:00.000Z',
    lastActivityAt: '2026-06-09T10:00:00.000Z',
    elapsedMs: 60_000,
    state: 'completed',
    lastAction: 'Update monitor',
    lastTail: null,
    pendingApproval: false,
    firstQuestion: null,
    lastQuestion: null,
    errorMessage: null,
    messageCount: 0,
    tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0, context: 0 },
    cost: { usd: null, confidence: 'derived' },
    quota: { fiveHourUsedPct: null, sevenDayUsedPct: null, resetAt: [], confidence: 'derived' },
    source: { confidence: 'derived', freshness: 'hot', lastUpdatedAt: '2026-06-09T10:00:00.000Z' },
    vcs: null,
    originLabel: null,
    originProvider: null,
    ...overrides,
  }
}

describe('island builders', () => {
  it('counts active, waiting, and unread completed runs', () => {
    const visited = new Set(['done-read'])
    const runs = [
      runFixture({ id: 'active-1', state: 'active' }),
      runFixture({ id: 'waiting-1', state: 'waitingApproval' }),
      runFixture({ id: 'waiting-2', pendingApproval: true }),
      runFixture({ id: 'done-unread', state: 'completed' }),
      runFixture({ id: 'done-read', state: 'completed' }),
      runFixture({ id: 'error-1', state: 'error' }),
    ]

    expect(buildIslandCounts(runs, visited)).toEqual({
      active: 1,
      waiting: 2,
      unreadDone: 1,
    })
  })

  it('sorts by status bucket, then latest activity within each bucket', () => {
    const visited = new Set(['done-read'])
    const runs = [
      runFixture({ id: 'done-read', state: 'completed', lastActivityAt: '2026-06-09T10:40:00.000Z' }),
      runFixture({ id: 'active-new', state: 'active', lastActivityAt: '2026-06-09T10:25:00.000Z' }),
      runFixture({ id: 'active-old', state: 'active', lastActivityAt: '2026-06-09T10:20:00.000Z' }),
      runFixture({ id: 'done-unread', state: 'completed', lastActivityAt: '2026-06-09T10:30:00.000Z' }),
      runFixture({ id: 'waiting-new', state: 'waitingApproval', lastActivityAt: '2026-06-09T10:15:00.000Z' }),
      runFixture({ id: 'waiting-old', state: 'waitingApproval', lastActivityAt: '2026-06-09T10:10:00.000Z' }),
      runFixture({ id: 'ignored-error', state: 'error', lastActivityAt: '2026-06-09T11:00:00.000Z' }),
    ]

    expect(buildIslandItems(runs, visited).map((item) => item.id)).toEqual([
      'waiting-new',
      'waiting-old',
      'active-new',
      'active-old',
      'done-unread',
      'done-read',
    ])
  })

  it('honors the item limit', () => {
    const runs = Array.from({ length: 10 }, (_, index) => runFixture({
      id: `active-${index}`,
      state: 'active',
      lastActivityAt: `2026-06-09T10:${String(index).padStart(2, '0')}:00.000Z`,
    }))

    expect(buildIslandItems(runs, new Set(), 3).map((item) => item.id)).toEqual([
      'active-9',
      'active-8',
      'active-7',
    ])
  })
})

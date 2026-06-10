import { describe, expect, it } from 'vitest'
import { applyMonitorFilters, runMatchesQuickFilter, runMatchesSearch, runMatchesToolFilter } from './monitorFilters'
import type { RunRecord } from './types'

function makeRun(overrides: Partial<RunRecord>): RunRecord {
  return {
    id: 'r',
    tool: 'codex',
    sourceMode: 'test',
    projectName: 'demo',
    workspacePath: '/tmp/demo',
    workspaceShort: '~/demo',
    model: null,
    provider: null,
    agentName: null,
    agentDisplayName: null,
    accountAlias: null,
    authMode: null,
    authVerified: false,
    sessionId: null,
    threadId: null,
    sessionKey: null,
    transcriptPath: null,
    startedAt: '2026-04-24T00:00:00Z',
    lastActivityAt: '2026-04-24T00:00:00Z',
    elapsedMs: 0,
    state: 'idle',
    lastAction: null,
    lastTail: null,
    pendingApproval: false,
    firstQuestion: null,
    lastQuestion: null,
    errorMessage: null,
    messageCount: 0,
    tokens: {
      input: 0,
      output: 0,
      cacheRead: 0,
      cacheWrite: 0,
      total: 0,
      context: 0,
    },
    cost: { usd: null, confidence: 'estimated' },
    quota: {
      fiveHourUsedPct: null,
      sevenDayUsedPct: null,
      resetAt: [],
      confidence: 'derived',
    },
    source: {
      confidence: 'live',
      freshness: 'hot',
      lastUpdatedAt: '2026-04-24T00:00:00Z',
    },
    vcs: null,
    originLabel: null,
    originProvider: null,
    ...overrides,
  }
}

describe('runMatchesQuickFilter', () => {
  it('all passes any state', () => {
    expect(runMatchesQuickFilter(makeRun({ state: 'idle' }), 'all')).toBe(true)
    expect(runMatchesQuickFilter(makeRun({ state: 'error' }), 'all')).toBe(true)
  })

  it('active only matches active state', () => {
    expect(runMatchesQuickFilter(makeRun({ state: 'active' }), 'active')).toBe(true)
    expect(runMatchesQuickFilter(makeRun({ state: 'idle' }), 'active')).toBe(false)
    expect(runMatchesQuickFilter(makeRun({ state: 'waitingApproval' }), 'active')).toBe(false)
  })

  it('attention matches waitingApproval and error-family states', () => {
    expect(runMatchesQuickFilter(makeRun({ state: 'waitingApproval' }), 'attention')).toBe(true)
    expect(runMatchesQuickFilter(makeRun({ state: 'error' }), 'attention')).toBe(true)
    expect(runMatchesQuickFilter(makeRun({ state: 'limitExceeded' }), 'attention')).toBe(true)
    expect(runMatchesQuickFilter(makeRun({ state: 'gatewayOffline' }), 'attention')).toBe(true)
    expect(runMatchesQuickFilter(makeRun({ state: 'contextExceeded' }), 'attention')).toBe(true)
    expect(runMatchesQuickFilter(makeRun({ state: 'active' }), 'attention')).toBe(false)
    expect(runMatchesQuickFilter(makeRun({ state: 'completed' }), 'attention')).toBe(false)
  })
})

describe('runMatchesSearch', () => {
  it('empty query matches everything', () => {
    expect(runMatchesSearch(makeRun({}), '')).toBe(true)
    expect(runMatchesSearch(makeRun({}), '   ')).toBe(true)
  })

  it('matches on projectName / workspaceShort / lastQuestion', () => {
    const run = makeRun({
      projectName: 'OctoMonitor',
      workspaceShort: '~/code/octo',
      lastQuestion: 'Fix login bug',
    })
    expect(runMatchesSearch(run, 'octo')).toBe(true)
    expect(runMatchesSearch(run, 'login')).toBe(true)
    expect(runMatchesSearch(run, 'fix')).toBe(true)
    expect(runMatchesSearch(run, 'unrelated')).toBe(false)
  })

  it('matches on tool label', () => {
    expect(runMatchesSearch(makeRun({ tool: 'claude' }), 'claude code')).toBe(true)
  })

  it('matches on run id prefix', () => {
    const run = makeRun({ id: 'codex-session-abcdef' })
    expect(runMatchesSearch(run, 'abcdef')).toBe(true)
  })

  it('search is case-insensitive', () => {
    const run = makeRun({ projectName: 'Hello' })
    expect(runMatchesSearch(run, 'HELLO')).toBe(true)
  })
})

describe('runMatchesToolFilter', () => {
  it('matches all or the exact tool only', () => {
    expect(runMatchesToolFilter(makeRun({ tool: 'codex' }), 'all')).toBe(true)
    expect(runMatchesToolFilter(makeRun({ tool: 'codex' }), 'codex')).toBe(true)
    expect(runMatchesToolFilter(makeRun({ tool: 'codex' }), 'claude')).toBe(false)
  })
})

describe('applyMonitorFilters', () => {
  it('short-circuits when filter is all and search empty', () => {
    const runs = [makeRun({ id: 'a' }), makeRun({ id: 'b' })]
    const out = applyMonitorFilters(runs, 'all', 'all', '')
    expect(out).toBe(runs) // same reference, no allocation
  })

  it('applies quick, tool, and search filters', () => {
    const runs = [
      makeRun({ id: 'a', state: 'active', projectName: 'alpha', tool: 'codex' }),
      makeRun({ id: 'b', state: 'active', projectName: 'beta' }),
      makeRun({ id: 'c', state: 'idle', projectName: 'alpha-other', tool: 'codex' }),
      makeRun({ id: 'd', state: 'active', projectName: 'alpha', tool: 'claude' }),
    ]
    const out = applyMonitorFilters(runs, 'active', 'codex', 'alpha')
    expect(out.map((r) => r.id)).toEqual(['a'])
  })
})

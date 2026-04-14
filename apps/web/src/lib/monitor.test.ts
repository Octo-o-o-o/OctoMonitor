import type { RunRecord } from './types'
import type { FilterRules } from './preferences'
import { buildMonitorStateStats, buildVisiblePanels, buildVisibleRunIds, buildVisibleRunsBySource } from './monitor'

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
      cacheRead: 0,
      cacheWrite: 0,
      total: 1_500,
      context: 0,
    },
    cost: {
      usd: 1.5,
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
    workflowHint: null,
    ...overrides,
  }
}

const defaultFilterRules: FilterRules = {
  claude: { mode: 'off', patterns: [] },
  codex: { mode: 'off', patterns: [] },
  openClaw: { mode: 'off', patterns: [] },
  hermes: { mode: 'off', patterns: [] },
}

describe('monitor visibility selectors', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-04-01T02:00:00.000Z'))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('keeps live sessions and recent completed sessions within the monitor period', () => {
    const visible = buildVisibleRunsBySource([
      createRun({
        id: 'active-codex',
        state: 'active',
        tool: 'codex',
        lastActivityAt: '2026-04-01T01:59:00.000Z',
      }),
      createRun({
        id: 'recent-claude',
        tool: 'claude',
        lastActivityAt: '2026-04-01T01:30:00.000Z',
      }),
      createRun({
        id: 'old-openclaw',
        tool: 'openClaw',
        lastActivityAt: '2026-03-31T18:00:00.000Z',
      }),
    ], defaultFilterRules, '1h')

    expect(visible.codex.map((run) => run.id)).toEqual(['active-codex'])
    expect(visible.claude.map((run) => run.id)).toEqual(['recent-claude'])
    expect(visible.openClaw).toEqual([])
  })

  it('counts only completed runs within the current monitor period for header stats', () => {
    const counts = buildMonitorStateStats([
      createRun({
        id: 'active-codex',
        state: 'active',
        lastActivityAt: '2026-04-01T01:59:00.000Z',
      }),
      createRun({
        id: 'waiting-claude',
        tool: 'claude',
        state: 'waitingApproval',
        lastActivityAt: '2026-03-31T12:00:00.000Z',
      }),
      createRun({
        id: 'recent-openclaw',
        tool: 'openClaw',
        lastActivityAt: '2026-04-01T01:15:00.000Z',
      }),
      createRun({
        id: 'old-done',
        tool: 'openClaw',
        lastActivityAt: '2026-03-31T18:00:00.000Z',
      }),
    ], '1h')

    expect(counts).toEqual({
      active: 1,
      waiting: 1,
      done: 1,
    })
  })

  it('applies panel order and filtering to keyboard-visible run ids', () => {
    const sessionsBySource = buildVisibleRunsBySource([
      createRun({
        id: 'codex-a',
        tool: 'codex',
        workspacePath: '/tmp/octomonitor',
        workspaceShort: 'octomonitor',
        lastActivityAt: '2026-04-01T01:55:00.000Z',
      }),
      createRun({
        id: 'claude-a',
        tool: 'claude',
        workspacePath: '/tmp/hidden',
        workspaceShort: 'hidden',
        lastActivityAt: '2026-04-01T01:50:00.000Z',
      }),
      createRun({
        id: 'openclaw-a',
        tool: 'openClaw',
        agentName: 'atlas',
        agentDisplayName: 'Atlas',
        lastActivityAt: '2026-04-01T01:45:00.000Z',
      }),
    ], {
      ...defaultFilterRules,
      claude: { mode: 'exclude', patterns: ['hidden'] },
    }, '1h')

    const visiblePanels = buildVisiblePanels([
      { tool: 'openClaw', enabled: true },
      { tool: 'codex', enabled: true },
      { tool: 'claude', enabled: true },
    ])

    expect(buildVisibleRunIds(sessionsBySource, visiblePanels, 'name')).toEqual([
      'openclaw-a',
      'codex-a',
    ])
  })
})

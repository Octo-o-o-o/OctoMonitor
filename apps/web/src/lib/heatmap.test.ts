import { buildHeatmapView, buildSummary, getHeatmapScopeSelection, listCommitsInWindow } from './heatmap'
import type { CommitRecord, RunRecord, UsageBucket } from './types'

function localIso(year: number, month: number, day: number, hour = 0, minute = 0): string {
  return new Date(year, month - 1, day, hour, minute, 0, 0).toISOString()
}

function createRun(overrides: Partial<RunRecord>): RunRecord {
  return {
    id: overrides.id ?? 'run-1',
    tool: overrides.tool ?? 'codex',
    sourceMode: overrides.sourceMode ?? 'history',
    projectName: overrides.projectName ?? 'OctoMonitor',
    workspacePath: overrides.workspacePath ?? '/tmp/octomonitor',
    workspaceShort: overrides.workspaceShort ?? '~/octomonitor',
    model: overrides.model ?? 'gpt-5',
    provider: overrides.provider ?? 'openai',
    agentName: overrides.agentName ?? null,
    agentDisplayName: overrides.agentDisplayName ?? null,
    accountAlias: overrides.accountAlias ?? null,
    authMode: overrides.authMode ?? null,
    authVerified: overrides.authVerified ?? true,
    sessionId: overrides.sessionId ?? null,
    threadId: overrides.threadId ?? null,
    sessionKey: overrides.sessionKey ?? null,
    transcriptPath: overrides.transcriptPath ?? null,
    startedAt: overrides.startedAt ?? localIso(2026, 4, 3, 13),
    lastActivityAt: overrides.lastActivityAt ?? localIso(2026, 4, 3, 15),
    elapsedMs: overrides.elapsedMs ?? 2 * 60 * 60 * 1000,
    state: overrides.state ?? 'completed',
    lastAction: overrides.lastAction ?? null,
    lastTail: overrides.lastTail ?? null,
    pendingApproval: overrides.pendingApproval ?? false,
    firstQuestion: overrides.firstQuestion ?? null,
    lastQuestion: overrides.lastQuestion ?? null,
    errorMessage: overrides.errorMessage ?? null,
    messageCount: overrides.messageCount ?? 4,
    tokens: overrides.tokens ?? {
      input: 80,
      output: 120,
      cacheRead: 0,
      cacheWrite: 0,
      total: 200,
      context: 0,
    },
    cost: overrides.cost ?? {
      usd: 1.2,
      confidence: 'estimated',
    },
    quota: overrides.quota ?? {
      fiveHourUsedPct: null,
      sevenDayUsedPct: null,
      resetAt: [],
      confidence: 'derived',
    },
    source: overrides.source ?? {
      confidence: 'derived',
      freshness: 'warm',
      lastUpdatedAt: localIso(2026, 4, 3, 15),
    },
    vcs: overrides.vcs ?? null,
    originLabel: overrides.originLabel ?? null,
    originProvider: overrides.originProvider ?? null,
  }
}

function createUsageBucket(overrides: Partial<UsageBucket>): UsageBucket {
  return {
    scope: overrides.scope ?? { runId: 'run-1' },
    window: overrides.window ?? 'hour',
    start: overrides.start ?? localIso(2026, 4, 3, 13),
    end: overrides.end ?? localIso(2026, 4, 3, 15),
    inputTokens: overrides.inputTokens ?? 80,
    outputTokens: overrides.outputTokens ?? 120,
    cacheReadTokens: overrides.cacheReadTokens ?? 0,
    cacheWriteTokens: overrides.cacheWriteTokens ?? 0,
    totalTokens: overrides.totalTokens ?? 200,
    costUsd: overrides.costUsd ?? 1.2,
    confidence: overrides.confidence ?? 'estimated',
  }
}

function createCommit(overrides: Partial<CommitRecord>): CommitRecord {
  return {
    id: overrides.id ?? 'commit-1',
    repoId: overrides.repoId ?? 'repo-1',
    repoName: overrides.repoName ?? 'OctoMonitor',
    repoRoot: overrides.repoRoot ?? '/tmp/octomonitor',
    worktreeId: overrides.worktreeId ?? null,
    worktreeName: overrides.worktreeName ?? null,
    sha: overrides.sha ?? 'abcdef1234567890',
    shortSha: overrides.shortSha ?? 'abcdef1',
    authorName: overrides.authorName ?? 'Yixiao',
    committedAt: overrides.committedAt ?? localIso(2026, 4, 3, 13, 30),
    summary: overrides.summary ?? 'Refine heatmap view',
    filesChanged: overrides.filesChanged ?? 3,
    insertions: overrides.insertions ?? 20,
    deletions: overrides.deletions ?? 4,
    attributedTokens: overrides.attributedTokens ?? 180,
    attributedCostUsd: overrides.attributedCostUsd ?? 0.8,
    runCount: overrides.runCount ?? 1,
    sourceCount: overrides.sourceCount ?? 1,
    confidence: overrides.confidence ?? 'heuristic',
    method: overrides.method ?? 'readOnlyHeuristic',
    sources: overrides.sources ?? [],
    links: overrides.links ?? [],
  }
}

describe('heatmap helpers', () => {
  it('builds a week range that includes the last 7 days', () => {
    const now = new Date(2026, 3, 3, 12, 0, 0, 0)
    const range = getHeatmapScopeSelection('week', now)

    expect(range.from.getTime()).toBe(new Date(2026, 2, 28, 0, 0, 0, 0).getTime())
    expect(range.to.getTime()).toBe(new Date(2026, 3, 3, 23, 59, 59, 999).getTime())
  })

  it('aggregates hourly input values from run usage slices', () => {
    const now = new Date(2026, 3, 3, 12, 0, 0, 0)
    const runs = [createRun({ id: 'run-1' })]
    const usageBuckets = [createUsageBucket({ scope: { runId: 'run-1' } })]

    const view = buildHeatmapView({
      scope: 'week',
      metric: 'input',
      now,
      runs,
      usageBuckets,
      commits: [],
    })

    expect(view.kind).toBe('hourly')
    if (view.kind !== 'hourly') {
      throw new Error('expected hourly heatmap')
    }
    const latestRow = view.rows[view.rows.length - 1]
    expect(latestRow.total).toBeCloseTo(4)
    expect(latestRow.cells[13]?.value).toBeCloseTo(2)
    expect(latestRow.cells[14]?.value).toBeCloseTo(2)
    expect(view.summary.total).toBeCloseTo(4)
    expect(view.summary.activeDays).toBe(1)
    expect(view.summary.longestStreak).toBe(1)
  })

  it('builds a calendar view and lists commits for the selected date window', () => {
    const now = new Date(2026, 3, 3, 12, 0, 0, 0)
    const commits = [
      createCommit({
        id: 'commit-a',
        shortSha: 'aaa1111',
        summary: 'Add week heatmap panel',
        committedAt: localIso(2026, 4, 2, 10, 15),
      }),
      createCommit({
        id: 'commit-b',
        shortSha: 'bbb2222',
        summary: 'Refine token legend',
        committedAt: localIso(2026, 4, 2, 18, 5),
      }),
      createCommit({
        id: 'commit-c',
        shortSha: 'ccc3333',
        summary: 'Polish status bar spacing',
        committedAt: localIso(2026, 4, 3, 9, 10),
      }),
    ]

    const view = buildHeatmapView({
      scope: 'total',
      metric: 'commit',
      now,
      runs: [],
      usageBuckets: [],
      commits,
    })

    expect(view.kind).toBe('calendar')
    if (view.kind !== 'calendar') {
      throw new Error('expected calendar heatmap')
    }

    const dayCell = view.cells.find(
      (cell) => !cell.hidden && cell.date.getFullYear() === 2026 && cell.date.getMonth() === 3 && cell.date.getDate() === 2,
    )
    expect(dayCell?.value).toBe(2)

    const matched = listCommitsInWindow(commits, dayCell!.startMs, dayCell!.endMs)
    expect(matched.map((commit) => commit.shortSha)).toEqual(['bbb2222', 'aaa1111'])
    expect(view.summary.topDay?.total).toBe(2)
  })

  it('returns undefined peakCell/topDay when given empty cells and dayTotals', () => {
    const summary = buildSummary([], [])
    expect(summary.peakCell).toBeUndefined()
    expect(summary.topDay).toBeUndefined()
    expect(summary.total).toBe(0)
    expect(summary.activeDays).toBe(0)
    expect(summary.longestStreak).toBe(0)
  })
})

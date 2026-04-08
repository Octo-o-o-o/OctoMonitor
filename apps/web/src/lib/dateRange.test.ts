import type { CommitRecord, RunRecord, UsageBucket } from './types'
import { buildCommitDateRange, buildUsageDateRange, endOfDay, startOfDay } from './dateRange'

const baseRun: RunRecord = {
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
  startedAt: '2026-03-01T10:00:00.000Z',
  lastActivityAt: '2026-03-02T12:00:00.000Z',
  elapsedMs: 1,
  state: 'completed',
  lastAction: null,
  lastTail: null,
  pendingApproval: false,
  firstQuestion: null,
  lastQuestion: null,
  errorMessage: null,
  messageCount: 0,
  tokens: { input: 1, output: 1, cacheRead: 0, cacheWrite: 0, total: 2, context: 0 },
  cost: { usd: 1, confidence: 'estimated' },
  quota: { fiveHourUsedPct: null, sevenDayUsedPct: null, resetAt: [], confidence: 'derived' },
  source: { confidence: 'live', freshness: 'hot', lastUpdatedAt: '2026-03-02T12:00:00.000Z' },
  vcs: null,
  originLabel: null,
  originProvider: null,
  workflowHint: null,
}

describe('date range helpers', () => {
  it('builds usage bounds from buckets first', () => {
    const bucket: UsageBucket = {
      scope: { runId: 'run-1', tool: 'codex' },
      window: 'session',
      start: '2026-02-15T09:00:00.000Z',
      end: '2026-03-20T18:00:00.000Z',
      inputTokens: 10,
      outputTokens: 5,
      cacheReadTokens: 0,
      cacheWriteTokens: 0,
      totalTokens: 15,
      costUsd: 1.5,
      confidence: 'estimated',
    }

    const range = buildUsageDateRange([baseRun], [bucket])

    expect(range?.from.getTime()).toBe(startOfDay(new Date('2026-02-15T09:00:00.000Z')).getTime())
    expect(range?.to.getTime()).toBe(endOfDay(new Date('2026-03-20T18:00:00.000Z')).getTime())
  })

  it('builds commit bounds from earliest to latest commit date', () => {
    const commits: CommitRecord[] = [
      {
        id: 'c1',
        repoId: 'repo',
        repoName: 'Repo',
        repoRoot: '/tmp/repo',
        worktreeId: null,
        worktreeName: null,
        sha: 'abc',
        shortSha: 'abc',
        authorName: 'A',
        committedAt: '2026-01-10T05:00:00.000Z',
        summary: 'First',
        filesChanged: 1,
        insertions: 1,
        deletions: 0,
        attributedTokens: 10,
        attributedCostUsd: 0.1,
        runCount: 1,
        sourceCount: 1,
        confidence: 'heuristic',
        method: 'readOnlyHeuristic',
        sources: [],
        links: [],
      },
      {
        id: 'c2',
        repoId: 'repo',
        repoName: 'Repo',
        repoRoot: '/tmp/repo',
        worktreeId: null,
        worktreeName: null,
        sha: 'def',
        shortSha: 'def',
        authorName: 'B',
        committedAt: '2026-04-01T20:00:00.000Z',
        summary: 'Last',
        filesChanged: 2,
        insertions: 3,
        deletions: 1,
        attributedTokens: 20,
        attributedCostUsd: 0.2,
        runCount: 1,
        sourceCount: 1,
        confidence: 'heuristic',
        method: 'readOnlyHeuristic',
        sources: [],
        links: [],
      },
    ]

    const range = buildCommitDateRange(commits)

    expect(range?.from.getTime()).toBe(startOfDay(new Date('2026-01-10T05:00:00.000Z')).getTime())
    expect(range?.to.getTime()).toBe(endOfDay(new Date('2026-04-01T20:00:00.000Z')).getTime())
  })
})

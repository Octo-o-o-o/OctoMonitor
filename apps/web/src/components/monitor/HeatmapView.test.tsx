import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { I18nProvider } from '../../lib/i18n'
import type { BootstrapPayload, CommitHistoryPayload, UsageHistoryPayload } from '../../lib/types'
import { useMonitorStore } from '../../store/monitorStore'
import { HeatmapView } from './HeatmapView'

const historyMocks = vi.hoisted(() => ({
  fetchUsageHistory: vi.fn<(_: unknown, __?: AbortSignal) => Promise<UsageHistoryPayload>>(),
  fetchCommitHistory: vi.fn<(_: unknown, __?: AbortSignal) => Promise<CommitHistoryPayload>>(),
}))

vi.mock('../../lib/history', () => ({
  fetchUsageHistory: historyMocks.fetchUsageHistory,
  fetchCommitHistory: historyMocks.fetchCommitHistory,
}))

function localIso(year: number, month: number, day: number, hour = 0, minute = 0): string {
  return new Date(year, month - 1, day, hour, minute, 0, 0).toISOString()
}

function createBootstrap(): BootstrapPayload {
  return {
    generatedAt: localIso(2026, 4, 3, 16),
    runs: [],
    attentions: [],
    usageBuckets: [],
    commits: [],
    identities: [],
    adapterHealth: [],
    recentCompletions: [],
    pendingCrons: [],
    config: {
      listenHost: '127.0.0.1',
      listenPort: 46321,
      historyDays: 30,
      companionEnabled: false,
      localIp: null,
      disabledSources: [],
      hiddenSources: [],
    },
  }
}

function createUsagePayload(): UsageHistoryPayload {
  return {
    generatedAt: localIso(2026, 4, 3, 16),
    range: {
      from: localIso(2025, 4, 4, 0),
      to: localIso(2026, 4, 3, 23, 59),
    },
    truncated: false,
    runs: [
      {
        id: 'run-1',
        tool: 'codex',
        sourceMode: 'history',
        projectName: 'OctoMonitor',
        workspacePath: '/tmp/octomonitor',
        workspaceShort: '~/octomonitor',
        model: 'gpt-5',
        provider: 'openai',
        agentName: null,
        agentDisplayName: null,
        accountAlias: null,
        authMode: null,
        authVerified: true,
        sessionId: 'session-1',
        threadId: null,
        sessionKey: null,
        transcriptPath: null,
        startedAt: localIso(2026, 4, 3, 13),
        lastActivityAt: localIso(2026, 4, 3, 15),
        elapsedMs: 2 * 60 * 60 * 1000,
        state: 'completed',
        lastAction: null,
        lastTail: null,
        pendingApproval: false,
        firstQuestion: null,
        lastQuestion: null,
        errorMessage: null,
        messageCount: 4,
        tokens: {
          input: 300,
          output: 500,
          cacheRead: 0,
          cacheWrite: 0,
          total: 800,
          context: 0,
        },
        cost: {
          usd: 1.4,
          confidence: 'estimated',
        },
        quota: {
          fiveHourUsedPct: null,
          sevenDayUsedPct: null,
          resetAt: [],
          confidence: 'derived',
        },
        source: {
          confidence: 'derived',
          freshness: 'warm',
          lastUpdatedAt: localIso(2026, 4, 3, 15),
        },
        vcs: null,
        originLabel: null,
        originProvider: null,
      },
    ],
    usageBuckets: [
      {
        scope: { runId: 'run-1' },
        window: 'hour',
        start: localIso(2026, 4, 3, 13),
        end: localIso(2026, 4, 3, 15),
        inputTokens: 300,
        outputTokens: 500,
        cacheReadTokens: 0,
        cacheWriteTokens: 0,
        totalTokens: 800,
        costUsd: 1.4,
        confidence: 'estimated',
      },
    ],
  }
}

function createCommitPayload(): CommitHistoryPayload {
  return {
    generatedAt: localIso(2026, 4, 3, 16),
    range: {
      from: localIso(2025, 4, 4, 0),
      to: localIso(2026, 4, 3, 23, 59),
    },
    truncated: false,
    runs: [],
    commits: [
      {
        id: 'commit-1',
        repoId: 'repo-1',
        repoName: 'OctoMonitor',
        repoRoot: '/tmp/octomonitor',
        worktreeId: null,
        worktreeName: null,
        sha: 'abcdef1234567890',
        shortSha: 'abcdef1',
        authorName: 'Yixiao',
        committedAt: localIso(2026, 4, 3, 13, 20),
        summary: 'Refine heatmap rendering',
        filesChanged: 3,
        insertions: 20,
        deletions: 6,
        attributedTokens: 400,
        attributedCostUsd: 0.6,
        runCount: 1,
        sourceCount: 1,
        confidence: 'heuristic',
        method: 'readOnlyHeuristic',
        sources: [],
        links: [],
      },
      {
        id: 'commit-2',
        repoId: 'repo-1',
        repoName: 'OctoMonitor',
        repoRoot: '/tmp/octomonitor',
        worktreeId: null,
        worktreeName: null,
        sha: '1234567890abcdef',
        shortSha: '1234567',
        authorName: 'Yixiao',
        committedAt: localIso(2026, 4, 3, 14, 10),
        summary: 'Tighten sidebar spacing',
        filesChanged: 2,
        insertions: 11,
        deletions: 2,
        attributedTokens: 260,
        attributedCostUsd: 0.3,
        runCount: 1,
        sourceCount: 1,
        confidence: 'heuristic',
        method: 'readOnlyHeuristic',
        sources: [],
        links: [],
      },
    ],
  }
}

describe('HeatmapView', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true })
    vi.setSystemTime(new Date(localIso(2026, 4, 3, 16)))
    historyMocks.fetchUsageHistory.mockResolvedValue(createUsagePayload())
    historyMocks.fetchCommitHistory.mockResolvedValue(createCommitPayload())

    act(() => {
      useMonitorStore.setState({
        data: createBootstrap(),
        connectionStatus: 'live',
        activeTab: 'heatmap',
        selectedRunId: undefined,
        focusedRunId: undefined,
      })
    })
  })

  afterEach(() => {
    vi.useRealTimers()
    historyMocks.fetchUsageHistory.mockReset()
    historyMocks.fetchCommitHistory.mockReset()
    act(() => {
      useMonitorStore.setState({
        data: null,
        connectionStatus: 'connecting',
        activeTab: 'monitor',
        selectedRunId: undefined,
        focusedRunId: undefined,
      })
    })
  })

  it('pins selection on click and prevents hover from overriding it', async () => {
    await act(async () => {
      render(<I18nProvider><HeatmapView /></I18nProvider>)
    })

    await waitFor(() => expect(historyMocks.fetchUsageHistory).toHaveBeenCalled())
    fireEvent.click(screen.getByRole('button', { name: 'WEEK x HOUR' }))

    const hour13 = await screen.findByRole('button', { name: /13:00 - 14:00 400/ })
    const hour14 = await screen.findByRole('button', { name: /14:00 - 15:00 400/ })

    fireEvent.mouseEnter(hour13)
    expect(await screen.findByText('Refine heatmap rendering')).toBeInTheDocument()
    expect(screen.queryByText('Tighten sidebar spacing')).not.toBeInTheDocument()
    expect(screen.getByText('PREVIEW')).toBeInTheDocument()

    fireEvent.click(hour13)
    expect(screen.getByText('PINNED')).toBeInTheDocument()

    fireEvent.mouseEnter(hour14)
    expect(screen.getByText('Refine heatmap rendering')).toBeInTheDocument()
    expect(screen.queryByText('Tighten sidebar spacing')).not.toBeInTheDocument()

    fireEvent.click(hour14)
    expect(await screen.findByText('Tighten sidebar spacing')).toBeInTheDocument()
    expect(screen.queryByText('Refine heatmap rendering')).not.toBeInTheDocument()

    fireEvent.click(hour14)
    expect(screen.getByText('PREVIEW')).toBeInTheDocument()

    fireEvent.mouseEnter(hour13)
    expect(await screen.findByText('Refine heatmap rendering')).toBeInTheDocument()
  })
})

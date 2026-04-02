import { act, fireEvent, render, screen } from '@testing-library/react'
import type { BootstrapPayload } from '../../lib/types'
import { I18nProvider } from '../../lib/i18n'
import { useMonitorStore } from '../../store/monitorStore'
import { CommitsView } from './CommitsView'

vi.mock('./DateRangePicker', () => ({
  DateRangePicker: () => <div data-testid="date-range-picker" />,
}))

function createBootstrap(): BootstrapPayload {
  const now = Date.now()
  const isoAgo = (ms: number) => new Date(now - ms).toISOString()

  return {
    generatedAt: isoAgo(0),
    runs: [
      {
        id: 'run-1',
        tool: 'claude',
        sourceMode: 'history',
        projectName: 'OctoMonitor',
        workspacePath: '/tmp/octomonitor',
        workspaceShort: '~/octomonitor',
        model: 'claude-sonnet-4.5',
        provider: 'anthropic',
        agentName: null,
        agentDisplayName: null,
        accountAlias: null,
        authMode: null,
        authVerified: true,
        sessionId: 'session-1',
        threadId: null,
        sessionKey: null,
        transcriptPath: null,
        startedAt: isoAgo(2 * 60 * 60 * 1000),
        lastActivityAt: isoAgo(80 * 60 * 1000),
        elapsedMs: 2_400_000,
        state: 'completed',
        lastAction: 'commit attribution UI',
        lastTail: null,
        pendingApproval: false,
        firstQuestion: 'review commit attribution',
        lastQuestion: 'refine worktree scoring',
        errorMessage: null,
        messageCount: 3,
        tokens: {
          input: 1200,
          output: 900,
          cacheRead: 0,
          cacheWrite: 0,
          total: 2100,
          context: 0,
        },
        cost: {
          usd: 3.2,
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
        lastUpdatedAt: isoAgo(80 * 60 * 1000),
        },
        vcs: {
          repoId: 'repo-1',
          repoName: 'OctoMonitor',
          repoRoot: '/tmp/octomonitor',
          worktreeId: 'wt-feature',
          worktreeName: 'feature-wt',
          worktreePath: '/tmp/octomonitor-feature',
          branch: 'feature/commit-attribution',
          confidence: 'derived',
        },
        originLabel: null,
        originProvider: null,
      },
      {
        id: 'run-2',
        tool: 'codex',
        sourceMode: 'live',
        projectName: 'SuperNode',
        workspacePath: '/tmp/supernode',
        workspaceShort: '~/supernode',
        model: 'gpt-5-codex',
        provider: 'openai',
        agentName: null,
        agentDisplayName: null,
        accountAlias: null,
        authMode: null,
        authVerified: true,
        sessionId: 'session-2',
        threadId: null,
        sessionKey: null,
        transcriptPath: null,
        startedAt: isoAgo(3 * 60 * 60 * 1000),
        lastActivityAt: isoAgo(160 * 60 * 1000),
        elapsedMs: 1_200_000,
        state: 'completed',
        lastAction: 'supernode deploy docs',
        lastTail: null,
        pendingApproval: false,
        firstQuestion: 'align deploy docs',
        lastQuestion: 'trim release checklist',
        errorMessage: null,
        messageCount: 2,
        tokens: {
          input: 600,
          output: 500,
          cacheRead: 0,
          cacheWrite: 0,
          total: 1100,
          context: 0,
        },
        cost: {
          usd: 1.3,
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
        lastUpdatedAt: isoAgo(160 * 60 * 1000),
        },
        vcs: {
          repoId: 'repo-2',
          repoName: 'SuperNode',
          repoRoot: '/tmp/supernode',
          worktreeId: 'wt-main',
          worktreeName: 'SuperNode',
          worktreePath: '/tmp/supernode',
          branch: 'main',
          confidence: 'derived',
        },
        originLabel: null,
        originProvider: null,
      },
    ],
    attentions: [],
    usageBuckets: [],
    commits: [
      {
        id: 'repo-1:abc123',
        repoId: 'repo-1',
        repoName: 'OctoMonitor',
        repoRoot: '/tmp/octomonitor',
        worktreeId: 'wt-feature',
        worktreeName: 'feature-wt',
        sha: 'abc1234567890',
        shortSha: 'abc1234',
        authorName: 'Yixiao',
        committedAt: isoAgo(85 * 60 * 1000),
        summary: 'Refine commit attribution UI',
        filesChanged: 4,
        insertions: 120,
        deletions: 18,
        attributedTokens: 1400,
        attributedCostUsd: 2.1,
        runCount: 1,
        sourceCount: 1,
        confidence: 'heuristic',
        method: 'readOnlyHeuristic',
        sources: [
          {
            tool: 'claude',
            runCount: 1,
            attributedTokens: 1400,
            attributedCostUsd: 2.1,
            confidence: 'heuristic',
          },
        ],
        links: [
          {
            runId: 'run-1',
            tool: 'claude',
            sourceMode: 'history',
            projectName: 'OctoMonitor',
            sessionLabel: 'refine worktree scoring',
            score: 0.67,
            allocatedTokens: 1400,
            allocatedCostUsd: 2.1,
            confidence: 'heuristic',
            method: 'readOnlyHeuristic',
          },
        ],
      },
      {
        id: 'repo-2:def456',
        repoId: 'repo-2',
        repoName: 'SuperNode',
        repoRoot: '/tmp/supernode',
        worktreeId: 'wt-main',
        worktreeName: 'SuperNode',
        sha: 'def4567890123',
        shortSha: 'def4567',
        authorName: 'Yixiao',
        committedAt: isoAgo(170 * 60 * 1000),
        summary: 'Update release checklist',
        filesChanged: 2,
        insertions: 20,
        deletions: 4,
        attributedTokens: 900,
        attributedCostUsd: 1.1,
        runCount: 1,
        sourceCount: 1,
        confidence: 'heuristic',
        method: 'readOnlyHeuristic',
        sources: [
          {
            tool: 'codex',
            runCount: 1,
            attributedTokens: 900,
            attributedCostUsd: 1.1,
            confidence: 'heuristic',
          },
        ],
        links: [
          {
            runId: 'run-2',
            tool: 'codex',
            sourceMode: 'live',
            projectName: 'SuperNode',
            sessionLabel: 'trim release checklist',
            score: 0.74,
            allocatedTokens: 900,
            allocatedCostUsd: 1.1,
            confidence: 'heuristic',
            method: 'readOnlyHeuristic',
          },
        ],
      },
    ],
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
    },
  }
}

describe.skip('CommitsView', () => {
  beforeEach(() => {
    act(() => {
      useMonitorStore.setState({
        data: createBootstrap(),
        connectionStatus: 'live',
        activeTab: 'commits',
        selectedRunId: undefined,
        focusedRunId: undefined,
      })
    })
  })

  afterEach(() => {
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

  it('renders linked session allocations and worktree context for attributed commits', async () => {
    await act(async () => {
      render(<I18nProvider><CommitsView /></I18nProvider>)
    })

    expect(screen.getByText('Refine commit attribution UI')).toBeInTheDocument()
    expect(screen.getAllByText(/feature-wt/).length).toBeGreaterThan(0)
    expect(screen.queryByText(/^LINKED SESSIONS$/)).not.toBeInTheDocument()
    expect(screen.queryByText(/^SESSION$/)).not.toBeInTheDocument()
    expect(screen.getAllByText('ALLOCATION').length).toBeGreaterThan(0)
    expect(screen.getAllByText('1 SESSION').length).toBeGreaterThan(0)
    expect(screen.queryByText('refine worktree scoring')).not.toBeInTheDocument()

    fireEvent.click(screen.getAllByRole('button', { name: 'VIEW SESSIONS' })[0])

    expect(screen.getByText('SESSION DETAILS (1)')).toBeInTheDocument()
    expect(screen.getByText('refine worktree scoring')).toBeInTheDocument()
    expect(screen.getByText('67%')).toBeInTheDocument()
  })

  it('supports project switching only through the top project tabs', async () => {
    await act(async () => {
      render(<I18nProvider><CommitsView /></I18nProvider>)
    })

    expect(screen.getAllByText('ALL').length).toBeGreaterThan(0)
    expect(screen.getAllByText('SuperNode').length).toBeGreaterThan(0)
    expect(screen.getByRole('tab', { name: /SuperNode/ })).toHaveAttribute('title', '/tmp/supernode')
    expect(screen.queryByPlaceholderText('Search project, commit, session…')).not.toBeInTheDocument()
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('tab', { name: /SuperNode/ }))
    expect(screen.getByText('Update release checklist')).toBeInTheDocument()
    expect(screen.queryByText('Refine commit attribution UI')).not.toBeInTheDocument()
  })
})

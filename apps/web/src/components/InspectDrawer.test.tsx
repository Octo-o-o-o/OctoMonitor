import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { I18nProvider } from '../lib/i18n'
import { ThemeProvider } from '../lib/theme'
import type { BootstrapPayload, RunRecord } from '../lib/types'
import { useMonitorStore } from '../store/monitorStore'
import { InspectDrawer } from './InspectDrawer'

function runFixture(overrides: Partial<RunRecord>): RunRecord {
  return {
    id: 'run-x',
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
    startedAt: '2026-04-16T00:00:00.000Z',
    lastActivityAt: '2026-04-16T00:01:00.000Z',
    elapsedMs: 60_000,
    state: 'active',
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
      lastUpdatedAt: '2026-04-16T00:01:00.000Z',
    },
    vcs: null,
    originLabel: null,
    originProvider: null,
    ...overrides,
  }
}

function bootstrapWithRuns(runs: RunRecord[]): BootstrapPayload {
  return {
    generatedAt: '2026-04-16T00:00:00.000Z',
    runs,
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

function flushPromises() {
  return new Promise((resolve) => setTimeout(resolve, 0))
}

describe('InspectDrawer', () => {
  beforeEach(() => {
    act(() => {
      useMonitorStore.setState({
        data: null,
        selectedRunId: undefined,
        focusedRunId: undefined,
        connectionStatus: 'live',
        activeTab: 'monitor',
        visitedRunIds: new Set<string>(),
      })
    })
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  afterEach(() => {
    vi.restoreAllMocks()
    act(() => {
      useMonitorStore.setState({
        data: null,
        selectedRunId: undefined,
        focusedRunId: undefined,
        connectionStatus: 'connecting',
        activeTab: 'monitor',
        visitedRunIds: new Set<string>(),
      })
    })
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  })

  it('renders nothing when no run is selected', () => {
    const { container } = render(
      <I18nProvider>
        <ThemeProvider>
          <InspectDrawer />
        </ThemeProvider>
      </I18nProvider>,
    )
    // The drawer hides itself entirely; we expect the rendered tree to be
    // empty so the side panel doesn't claim screen space.
    expect(container.firstChild).toBeNull()
  })

  it('fetches the Codex events endpoint when a Codex run is selected in local mode', async () => {
    // Codex runs in local mode have a structured event timeline. The drawer
    // should hit /api/runs/{id}/events on mount and then poll; for this test
    // we only assert the initial call so we don't have to drive the timer.
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async (input) => {
        const url = typeof input === 'string' ? input : (input as Request | URL).toString()
        if (url.includes('/events')) {
          return new Response(
            JSON.stringify({ tool: 'codex', events: [], cursor: 0, reset: true }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          )
        }
        return new Response('null', { status: 503 })
      })

    const codexRun = runFixture({ id: 'codex-1', tool: 'codex' })

    act(() => {
      useMonitorStore.setState({
        data: bootstrapWithRuns([codexRun]),
        selectedRunId: 'codex-1',
      })
    })

    render(
      <I18nProvider>
        <ThemeProvider>
          <InspectDrawer />
        </ThemeProvider>
      </I18nProvider>,
    )

    await act(async () => { await flushPromises() })
    const calls = fetchSpy.mock.calls.map(([input]) =>
      typeof input === 'string' ? input : (input as Request | URL).toString(),
    )
    expect(
      calls.some((url) => url.includes(`/api/runs/${encodeURIComponent('codex-1')}/events`)),
      `expected an events fetch, saw: ${JSON.stringify(calls)}`,
    ).toBe(true)
  })

  it('falls back to the legacy /inspect endpoint for Claude runs', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async (input) => {
        const url = typeof input === 'string' ? input : (input as Request | URL).toString()
        if (url.includes('/inspect')) {
          return new Response(JSON.stringify({ entries: [] }), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        return new Response('null', { status: 503 })
      })

    const claudeRun = runFixture({ id: 'claude-1', tool: 'claude' })

    act(() => {
      useMonitorStore.setState({
        data: bootstrapWithRuns([claudeRun]),
        selectedRunId: 'claude-1',
      })
    })

    render(
      <I18nProvider>
        <ThemeProvider>
          <InspectDrawer />
        </ThemeProvider>
      </I18nProvider>,
    )

    await act(async () => { await flushPromises() })
    const calls = fetchSpy.mock.calls.map(([input]) =>
      typeof input === 'string' ? input : (input as Request | URL).toString(),
    )
    expect(
      calls.some((url) => url.includes(`/api/runs/${encodeURIComponent('claude-1')}/inspect`)),
      `expected a legacy inspect fetch, saw: ${JSON.stringify(calls)}`,
    ).toBe(true)
    // And it should NOT have hit /events for a non-Codex run.
    expect(calls.some((url) => url.includes('/events'))).toBe(false)
  })

  it('shows Open in Codex only in the desktop app and marks the run visited on click', async () => {
    const invoke = vi.fn<(_: string, __?: Record<string, unknown>) => Promise<unknown>>()
      .mockResolvedValue(undefined)
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke },
    })
    vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
      new Response('null', { status: 503, headers: { 'content-type': 'application/json' } }),
    )

    const codexRun = runFixture({
      id: 'codex-open',
      tool: 'codex',
      threadId: '019eacb8-5af1-7d41-beb4-052a48825afa',
    })

    act(() => {
      useMonitorStore.setState({
        data: bootstrapWithRuns([codexRun]),
        selectedRunId: 'codex-open',
      })
    })

    render(
      <I18nProvider>
        <ThemeProvider>
          <InspectDrawer />
        </ThemeProvider>
      </I18nProvider>,
    )

    fireEvent.click(screen.getByRole('button', { name: 'Open in Codex' }))

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith('open_external', {
        url: 'codex://threads/019eacb8-5af1-7d41-beb4-052a48825afa',
      })
    })
    expect(useMonitorStore.getState().visitedRunIds.has('codex-open')).toBe(true)
  })

  it('does not show the Codex open action outside Tauri', async () => {
    const codexRun = runFixture({
      id: 'codex-browser',
      tool: 'codex',
      threadId: '019eacb8-5af1-7d41-beb4-052a48825afa',
    })

    act(() => {
      useMonitorStore.setState({
        data: bootstrapWithRuns([codexRun]),
        selectedRunId: 'codex-browser',
      })
    })

    render(
      <I18nProvider>
        <ThemeProvider>
          <InspectDrawer />
        </ThemeProvider>
      </I18nProvider>,
    )

    expect(screen.queryByRole('button', { name: 'Open in Codex' })).not.toBeInTheDocument()
    await act(async () => { await flushPromises() })
  })
})

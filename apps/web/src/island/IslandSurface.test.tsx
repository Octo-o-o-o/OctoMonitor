import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { I18nProvider } from '../lib/i18n'
import { defaultSettings } from '../lib/preferences'
import type { RunRecord } from '../lib/types'
import { useMonitorStore } from '../store/monitorStore'
import { IslandSurface } from './IslandSurface'

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
    threadId: '019eacb8-5af1-7d41-beb4-052a48825afa',
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

describe('IslandSurface', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    localStorage.clear()
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    act(() => {
      useMonitorStore.setState({
        data: null,
        connectionStatus: 'live',
        selectedRunId: undefined,
        focusedRunId: undefined,
        visitedRunIds: new Set<string>(),
        settings: defaultSettings,
      })
    })
  })

  afterEach(() => {
    vi.useRealTimers()
    localStorage.clear()
    act(() => {
      useMonitorStore.setState({
        data: null,
        connectionStatus: 'connecting',
        selectedRunId: undefined,
        focusedRunId: undefined,
        visitedRunIds: new Set<string>(),
        settings: defaultSettings,
      })
    })
  })

  it('expands after hover debounce and renders prioritized rows', () => {
    const runs = [
      runFixture({ id: 'done-1', projectName: 'Done Project', lastAction: 'Finished build' }),
      runFixture({ id: 'active-1', state: 'active', projectName: 'Active Project', lastAction: 'Running tests' }),
      runFixture({ id: 'waiting-1', state: 'waitingApproval', projectName: 'Waiting Project', lastQuestion: 'Approve shell command?' }),
    ]

    const { container } = render(
      <I18nProvider>
        <IslandSurface runs={runs} visitedRunIds={new Set()} connected />
      </I18nProvider>,
    )

    const shell = container.querySelector('.island-shell')
    expect(shell).not.toHaveClass('is-expanded')

    fireEvent.mouseEnter(shell!)
    act(() => {
      vi.advanceTimersByTime(130)
    })

    expect(shell).toHaveClass('is-expanded')
    expect(screen.getByText('Waiting Project')).toBeInTheDocument()
    expect(screen.getByText('Active Project')).toBeInTheDocument()
    expect(screen.getByText('Done Project')).toBeInTheDocument()
  })

  it('marks an item visited on click without opening external URLs outside Tauri', () => {
    const done = runFixture({ id: 'done-1', projectName: 'Done Project' })
    render(
      <I18nProvider>
        <IslandSurface runs={[done]} visitedRunIds={new Set()} connected />
      </I18nProvider>,
    )

    fireEvent.click(screen.getByRole('button', { name: /Done Project/i }))

    expect(useMonitorStore.getState().visitedRunIds.has('done-1')).toBe(true)
  })
})

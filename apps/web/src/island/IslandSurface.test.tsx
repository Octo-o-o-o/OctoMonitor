import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { I18nProvider } from '../lib/i18n'
import { defaultSettings } from '../lib/preferences'
import type { RunRecord } from '../lib/types'
import { useMonitorStore } from '../store/monitorStore'
import { IslandSurface } from './IslandSurface'

type IslandExpansionTestWindow = Window & {
  __OCTOMONITOR_ISLAND_EXPANDED__?: boolean
  __TAURI_INTERNALS__?: {
    invoke?: (command: string, payload?: Record<string, unknown>) => Promise<unknown>
  }
}

function islandExpansionWindow(): IslandExpansionTestWindow {
  return window as IslandExpansionTestWindow
}

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
    window.history.pushState({}, '', '/')
    localStorage.clear()
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    delete islandExpansionWindow().__OCTOMONITOR_ISLAND_EXPANDED__
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
    window.history.pushState({}, '', '/')
    localStorage.clear()
    delete islandExpansionWindow().__OCTOMONITOR_ISLAND_EXPANDED__
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
      vi.advanceTimersByTime(160)
    })

    expect(shell).toHaveClass('is-expanded')
    expect(screen.getByText('Waiting Project')).toBeInTheDocument()
    expect(screen.getByText('Active Project')).toBeInTheDocument()
    expect(screen.getByText('Done Project')).toBeInTheDocument()
    expect(screen.getByText('Action')).toBeInTheDocument()
    expect(screen.getByText('Running')).toBeInTheDocument()
    expect(screen.getByText('Just done')).toBeInTheDocument()
  })

  it('uses native notch metrics for the collapsed chrome', () => {
    window.history.pushState({}, '', '/?surface=island&closedWidth=277&closedHeight=32&notched=1')

    const { container } = render(
      <I18nProvider>
        <IslandSurface runs={[]} visitedRunIds={new Set()} connected />
      </I18nProvider>,
    )

    const shell = container.querySelector('.island-shell') as HTMLElement
    expect(shell).toHaveClass('is-notched')
    expect(shell.style.getPropertyValue('--island-width')).toBe('277px')
    expect(shell.style.getPropertyValue('--island-collapsed-height')).toBe('32px')
  })

  it('expands from the native desktop hover event', () => {
    const { container } = render(
      <I18nProvider>
        <IslandSurface runs={[]} visitedRunIds={new Set()} connected />
      </I18nProvider>,
    )

    const shell = container.querySelector('.island-shell')
    expect(shell).not.toHaveClass('is-expanded')

    act(() => {
      window.dispatchEvent(new CustomEvent('octomonitor-island-expansion', {
        detail: { expanded: true },
      }))
      vi.advanceTimersByTime(160)
    })

    expect(shell).toHaveClass('is-expanded')

    act(() => {
      window.dispatchEvent(new CustomEvent('octomonitor-island-expansion', {
        detail: { expanded: false },
      }))
      vi.advanceTimersByTime(300)
    })

    expect(shell).not.toHaveClass('is-expanded')
  })

  it('collapses immediately from a native outside-click event', () => {
    const { container } = render(
      <I18nProvider>
        <IslandSurface runs={[]} visitedRunIds={new Set()} connected />
      </I18nProvider>,
    )

    const shell = container.querySelector('.island-shell')
    act(() => {
      window.dispatchEvent(new CustomEvent('octomonitor-island-expansion', {
        detail: { expanded: true },
      }))
      vi.advanceTimersByTime(160)
    })
    expect(shell).toHaveClass('is-expanded')

    act(() => {
      window.dispatchEvent(new CustomEvent('octomonitor-island-expansion', {
        detail: { expanded: false, immediate: true },
      }))
    })

    expect(shell).not.toHaveClass('is-expanded')
  })

  it('uses the cached native hover state when the event fired before mount', () => {
    islandExpansionWindow().__OCTOMONITOR_ISLAND_EXPANDED__ = true

    const { container } = render(
      <I18nProvider>
        <IslandSurface runs={[]} visitedRunIds={new Set()} connected />
      </I18nProvider>,
    )

    const shell = container.querySelector('.island-shell')
    act(() => {
      vi.advanceTimersByTime(160)
    })

    expect(shell).toHaveClass('is-expanded')
  })

  it('opens desktop settings from the expanded header button', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke },
    })

    const { container } = render(
      <I18nProvider>
        <IslandSurface runs={[]} visitedRunIds={new Set()} connected />
      </I18nProvider>,
    )

    const shell = container.querySelector('.island-shell')
    act(() => {
      fireEvent.mouseEnter(shell!)
      vi.advanceTimersByTime(160)
    })

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /open settings/i }))
    })

    expect(invoke).toHaveBeenCalledWith('open_dashboard_settings')
    expect(shell).not.toHaveClass('is-expanded')
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

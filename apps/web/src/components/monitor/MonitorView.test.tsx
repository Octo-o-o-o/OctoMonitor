import { act, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { MonitorView } from './MonitorView'
import { I18nProvider } from '../../lib/i18n'
import { defaultSettings } from '../../lib/preferences'
import { STORAGE_KEYS } from '../../lib/storageKeys'
import type { BootstrapPayload, RunRecord } from '../../lib/types'
import { useMonitorStore } from '../../store/monitorStore'

function createBootstrap(): BootstrapPayload {
  return {
    generatedAt: '2026-04-16T00:00:00.000Z',
    runs: [],
    attentions: [],
    usageBuckets: [],
    commits: [],
    identities: [],
    adapterHealth: [
      {
        tool: 'openClaw',
        mode: 'gateway+status+probe',
        online: true,
        gatewayStatus: 'running',
        gatewayDetail: 'service running (active) | rpc reachable',
        lastSuccessAt: '2026-04-16T00:00:00.000Z',
        lastErrorAt: null,
        lastError: null,
        freshness: 'hot',
      },
      {
        tool: 'hermes',
        mode: 'sessions-scan+probe',
        online: true,
        gatewayStatus: 'stopped',
        gatewayDetail: 'default: stopped',
        lastSuccessAt: '2026-04-16T00:00:00.000Z',
        lastErrorAt: null,
        lastError: null,
        freshness: 'hot',
      },
    ],
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

function runFixture(overrides: Partial<RunRecord>): RunRecord {
  const now = new Date().toISOString()
  return {
    id: 'run-x',
    tool: 'codex',
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
    startedAt: now,
    lastActivityAt: now,
    elapsedMs: 60_000,
    state: 'completed',
    lastAction: 'Fix parser',
    lastTail: null,
    pendingApproval: false,
    firstQuestion: null,
    lastQuestion: null,
    errorMessage: null,
    messageCount: 0,
    tokens: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0, context: 0 },
    cost: { usd: null, confidence: 'derived' },
    quota: { fiveHourUsedPct: null, sevenDayUsedPct: null, resetAt: [], confidence: 'derived' },
    source: { confidence: 'derived', freshness: 'warm', lastUpdatedAt: now },
    vcs: null,
    originLabel: null,
    originProvider: null,
    ...overrides,
  }
}

describe('MonitorView gateway status', () => {
  beforeEach(() => {
    localStorage.clear()
    localStorage.setItem(STORAGE_KEYS.locale, 'en')

    act(() => {
      useMonitorStore.setState({
        data: createBootstrap(),
        connectionStatus: 'live',
        activeTab: 'monitor',
        showShortcutHelp: false,
        selectedRunId: undefined,
        focusedRunId: undefined,
        visitedRunIds: new Set<string>(),
        settings: {
          ...defaultSettings,
          panelConfig: [
            { tool: 'claude', enabled: false },
            { tool: 'codex', enabled: false },
            { tool: 'openClaw', enabled: true },
            { tool: 'hermes', enabled: true },
          ],
        },
      })
    })
  })

  afterEach(() => {
    act(() => {
      useMonitorStore.setState({
        data: null,
        connectionStatus: 'connecting',
        activeTab: 'monitor',
        showShortcutHelp: false,
        selectedRunId: undefined,
        focusedRunId: undefined,
        visitedRunIds: new Set<string>(),
        settings: defaultSettings,
      })
    })
    localStorage.clear()
  })

  it('renders gateway status badges for OpenClaw and Hermes', () => {
    render(
      <I18nProvider>
        <MonitorView />
      </I18nProvider>,
    )

    expect(screen.getByText('OPENCLAW')).toBeInTheDocument()
    expect(screen.getByText('HERMES')).toBeInTheDocument()
    expect(screen.getByText('RUNNING')).toBeInTheDocument()
    expect(screen.getByText('STOPPED')).toBeInTheDocument()
  })

  it('marks completed rows as visited and removes the unread dot on click', () => {
    const completed = runFixture({ id: 'done-1', lastAction: 'Fix parser' })
    act(() => {
      useMonitorStore.setState({
        data: { ...createBootstrap(), runs: [completed] },
        settings: {
          ...defaultSettings,
          panelConfig: [
            { tool: 'claude', enabled: false },
            { tool: 'codex', enabled: true },
            { tool: 'openClaw', enabled: false },
            { tool: 'hermes', enabled: false },
          ],
        },
      })
    })

    const { container } = render(
      <I18nProvider>
        <MonitorView />
      </I18nProvider>,
    )

    const row = screen.getByText('Fix parser').closest('button')
    expect(row).not.toBeNull()
    expect(row).not.toHaveClass('is-visited')
    expect(container.querySelector('.session-unvisited-dot')).not.toBeNull()

    fireEvent.click(row!)

    expect(useMonitorStore.getState().visitedRunIds.has('done-1')).toBe(true)
    expect(row).toHaveClass('is-visited')
    expect(container.querySelector('.session-unvisited-dot')).toBeNull()
  })
})

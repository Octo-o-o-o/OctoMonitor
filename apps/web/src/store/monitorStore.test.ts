import { act } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import type { BootstrapPayload, RunRecord } from '../lib/types'
import { STORAGE_KEYS } from '../lib/storageKeys'
import { selectSelectedRun, useMonitorStore } from './monitorStore'

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
    },
  }
}

function runRecord(overrides: Partial<RunRecord>): RunRecord {
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
    source: { confidence: 'derived', freshness: 'warm', lastUpdatedAt: '2026-04-16T00:01:00.000Z' },
    vcs: null,
    originLabel: null,
    originProvider: null,
    ...overrides,
  }
}

describe('monitorStore', () => {
  beforeEach(() => {
    localStorage.clear()
    act(() => {
      useMonitorStore.setState({
        data: null,
        selectedRunId: undefined,
        focusedRunId: undefined,
        connectionStatus: 'connecting',
        activeTab: 'monitor',
        showShortcutHelp: false,
        acknowledgedErrors: new Set<string>(),
        dismissedAttentionKeys: new Set<string>(),
        monitorQuickFilter: 'all',
        monitorSearch: '',
      })
    })
  })

  afterEach(() => {
    localStorage.clear()
  })

  describe('selectSelectedRun', () => {
    it('returns undefined when no run is selected', () => {
      const run = selectSelectedRun(useMonitorStore.getState())
      expect(run).toBeUndefined()
    })

    it('returns undefined when data is null even if selectedRunId is set', () => {
      // Cover the dangling-selection case (e.g. WS disconnect drops data
      // while the URL still references the previous run).
      act(() => {
        useMonitorStore.setState({ data: null, selectedRunId: 'run-1' })
      })
      const run = selectSelectedRun(useMonitorStore.getState())
      expect(run).toBeUndefined()
    })

    it('returns the matching run when selected', () => {
      act(() => {
        useMonitorStore.setState({
          data: bootstrapWithRuns([runRecord({ id: 'run-1' }), runRecord({ id: 'run-2' })]),
          selectedRunId: 'run-2',
        })
      })
      const run = selectSelectedRun(useMonitorStore.getState())
      expect(run?.id).toBe('run-2')
    })
  })

  describe('dismissAttention', () => {
    it('accumulates multiple dismissals and persists to localStorage', () => {
      act(() => {
        useMonitorStore.getState().dismissAttention('alert-1')
        useMonitorStore.getState().dismissAttention('alert-2')
      })
      const keys = useMonitorStore.getState().dismissedAttentionKeys
      expect(keys.has('alert-1')).toBe(true)
      expect(keys.has('alert-2')).toBe(true)
      // Storage carries the same set so it survives reload.
      const raw = localStorage.getItem(STORAGE_KEYS.dismissedAttentions)
      expect(raw).not.toBeNull()
      expect(JSON.parse(raw!)).toEqual(expect.arrayContaining(['alert-1', 'alert-2']))
    })

    it('keeps the in-memory set when localStorage write throws', () => {
      // Simulate a quota-exceeded / private-mode storage by stubbing setItem
      // to throw. The store's try/catch should swallow it but the runtime
      // dismissed-set must still update so the UI stays consistent.
      const originalSetItem = localStorage.setItem
      localStorage.setItem = () => {
        throw new Error('quota exceeded')
      }
      try {
        act(() => {
          useMonitorStore.getState().dismissAttention('alert-3')
        })
        expect(useMonitorStore.getState().dismissedAttentionKeys.has('alert-3')).toBe(true)
      } finally {
        localStorage.setItem = originalSetItem
      }
    })
  })

  describe('updateSettings', () => {
    it('merges patches and writes the combined snapshot to storage', () => {
      const initial = useMonitorStore.getState().settings
      act(() => {
        useMonitorStore.getState().updateSettings({ uiDensity: 'comfortable' })
        useMonitorStore.getState().updateSettings({ fontSize: 'large' })
      })
      const merged = useMonitorStore.getState().settings
      expect(merged.uiDensity).toBe('comfortable')
      expect(merged.fontSize).toBe('large')
      // Patches should leave untouched fields intact so consumers can update
      // a single setting without supplying the entire payload.
      expect(merged.locale).toBe(initial.locale)
    })
  })
})

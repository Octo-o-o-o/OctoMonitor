import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { MonitorView } from './MonitorView'
import { I18nProvider } from '../../lib/i18n'
import { defaultSettings } from '../../lib/preferences'
import { STORAGE_KEYS } from '../../lib/storageKeys'
import type { BootstrapPayload } from '../../lib/types'
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
})

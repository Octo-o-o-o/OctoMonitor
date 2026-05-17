import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { I18nProvider } from '../../lib/i18n'
import { ThemeProvider } from '../../lib/theme'
import type { BootstrapPayload } from '../../lib/types'
import { useMonitorStore } from '../../store/monitorStore'
import { StatusBar } from './StatusBar'

function bootstrapWithConfig(): BootstrapPayload {
  return {
    generatedAt: '2026-04-16T00:00:00.000Z',
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
    },
  }
}

function renderStatusBar({ wsConnected }: { wsConnected: boolean }) {
  return render(
    <I18nProvider>
      <ThemeProvider>
        <StatusBar runtimeMode="local" wsConnected={wsConnected} />
      </ThemeProvider>
    </I18nProvider>,
  )
}

describe('StatusBar', () => {
  beforeEach(() => {
    act(() => {
      useMonitorStore.setState({
        data: bootstrapWithConfig(),
        connectionStatus: 'live',
        activeTab: 'monitor',
        monitorQuickFilter: 'all',
        monitorSearch: '',
      })
    })
  })

  afterEach(() => {
    act(() => {
      useMonitorStore.setState({
        data: null,
        connectionStatus: 'connecting',
        activeTab: 'monitor',
      })
    })
  })

  it('renders the live indicator with connected styling when ws is up', () => {
    const { container } = renderStatusBar({ wsConnected: true })
    const indicator = container.querySelector('.live-indicator')
    expect(indicator).not.toBeNull()
    expect(indicator?.className).toContain('connected')
    expect(indicator?.className).not.toContain('disconnected')
  })

  it('renders the live indicator with disconnected styling and the offline label when ws is down', () => {
    // This is the LIVE/OFFLINE indicator the user relies on to decide whether
    // they're looking at stale snapshot data — must visibly flip.
    const { container } = renderStatusBar({ wsConnected: false })
    const indicator = container.querySelector('.live-indicator')
    expect(indicator?.className).toContain('disconnected')
    // The text content uses i18n; assert role-driven presence rather than the
    // exact string so the test isn't tied to a single locale's wording.
    expect(indicator?.textContent?.toUpperCase()).toContain('OFFLINE')
  })

  it('exposes the primary tabs as accessible tab buttons', () => {
    renderStatusBar({ wsConnected: true })
    // MONITOR / USAGE / COMMITS / HEATMAP / SETTINGS are the five product
    // surfaces the StatusBar gates access to — keyboard / a11y users navigate
    // by role=tab.
    const tabs = screen.queryAllByRole('tab')
    expect(tabs.length).toBeGreaterThanOrEqual(3)
  })
})

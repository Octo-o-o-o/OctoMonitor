import { act, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App'
import { DESKTOP_MENU_ACTION_EVENT } from './lib/desktopEvents'
import { I18nProvider } from './lib/i18n'
import { ThemeProvider } from './lib/theme'
import type { BootstrapPayload } from './lib/types'
import { useMonitorStore } from './store/monitorStore'

function createBootstrap(): BootstrapPayload {
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

describe('App', () => {
  beforeEach(() => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {
        invoke: vi.fn().mockResolvedValue(undefined),
      },
    })

    act(() => {
      useMonitorStore.setState({
        data: createBootstrap(),
        connectionStatus: 'live',
        activeTab: 'monitor',
        showShortcutHelp: false,
        selectedRunId: undefined,
        focusedRunId: undefined,
      })
    })
  })

  afterEach(() => {
    Reflect.deleteProperty(window, '__TAURI_INTERNALS__')
    act(() => {
      useMonitorStore.setState({
        data: null,
        connectionStatus: 'connecting',
        activeTab: 'monitor',
        showShortcutHelp: false,
        selectedRunId: undefined,
        focusedRunId: undefined,
      })
    })
  })

  it('renders monitor view by default', async () => {
    render(<I18nProvider><ThemeProvider><App /></ThemeProvider></I18nProvider>)
    expect(await screen.findByText('MONITOR')).toBeInTheDocument()
    expect(await screen.findByText('USAGE')).toBeInTheDocument()
    expect(await screen.findByText('COMMITS')).toBeInTheDocument()
    expect(await screen.findByText('HEATMAP')).toBeInTheDocument()
    expect(await screen.findByText('SETTINGS')).toBeInTheDocument()
  })

  it('opens settings when the desktop menu requests it', async () => {
    render(<I18nProvider><ThemeProvider><App /></ThemeProvider></I18nProvider>)

    act(() => {
      window.dispatchEvent(new CustomEvent(DESKTOP_MENU_ACTION_EVENT, {
        detail: { action: 'open-settings' },
      }))
    })

    expect(await screen.findByText('Environment & Doctor')).toBeInTheDocument()
  })

  it('toggles the shortcut overlay when the desktop menu requests it', async () => {
    render(<I18nProvider><ThemeProvider><App /></ThemeProvider></I18nProvider>)

    act(() => {
      window.dispatchEvent(new CustomEvent(DESKTOP_MENU_ACTION_EVENT, {
        detail: { action: 'toggle-shortcuts' },
      }))
    })

    expect(await screen.findByText('Keyboard Shortcuts')).toBeInTheDocument()
  })
})

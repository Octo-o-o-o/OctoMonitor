import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SettingsView } from './SettingsView'
import { I18nProvider } from '../../lib/i18n'
import { ThemeProvider } from '../../lib/theme'

const remoteAccessState = {
  enabled: true,
  mode: 'lanViewer',
  listenerHost: '0.0.0.0',
  listenerPort: 46322,
  addresses: [
    { kind: 'lan', label: 'LAN', url: 'http://192.168.1.20:46322' },
  ],
  devices: [],
  pendingPairings: [],
}

describe('SettingsView', () => {
  beforeEach(() => {
    localStorage.clear()
    localStorage.setItem('octomonitor-locale', 'zh')
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify(remoteAccessState), {
        status: 200,
        headers: { 'Content-Type': 'application/json' },
      }),
    )
  })

  afterEach(() => {
    vi.restoreAllMocks()
    localStorage.clear()
  })

  it('keeps the remote access section visible near the top of settings', async () => {
    const { container } = render(
      <I18nProvider>
        <ThemeProvider>
          <SettingsView />
        </ThemeProvider>
      </I18nProvider>,
    )

    const labels = Array.from(container.querySelectorAll('.settings-section > .section-label'))
      .map((node) => node.textContent?.trim())

    expect(labels.indexOf('远程访问')).toBeGreaterThan(-1)
    expect(labels.indexOf('远程访问')).toBeLessThan(labels.indexOf('过滤规则'))
    expect(screen.getByRole('switch', { name: '允许远程只读访问' })).toBeInTheDocument()
    expect(await screen.findByText('生成配对码')).toBeInTheDocument()
  })
})

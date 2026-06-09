import { render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { SettingsView } from './SettingsView'
import { I18nProvider } from '../../lib/i18n'
import { STORAGE_KEYS } from '../../lib/storageKeys'
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

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'Content-Type': 'application/json' },
  })
}

describe('SettingsView', () => {
  beforeEach(() => {
    localStorage.clear()
    localStorage.setItem(STORAGE_KEYS.locale, 'zh')
    // Use mockImplementation so each fetch call yields a *fresh* Response —
    // `mockResolvedValue` returns the same Response instance every time, and
    // its body can only be read once (`Body has already been read`).
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input) => {
      const url = typeof input === 'string' ? input : (input as Request | URL).toString()
      if (url.endsWith('/api/remote/access')) {
        return jsonResponse(remoteAccessState)
      }
      if (url.endsWith('/api/installer/detect')) {
        return jsonResponse({ capabilities: [] })
      }
      if (url.endsWith('/api/installer/doctor')) {
        return jsonResponse({ checks: [] })
      }
      return jsonResponse(null, 503)
    })
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
    expect(labels.indexOf('桌面显示')).toBeGreaterThan(-1)
    expect(labels.indexOf('桌面显示')).toBeLessThan(labels.indexOf('远程访问'))
    expect(labels.indexOf('远程访问')).toBeLessThan(labels.indexOf('过滤规则'))
    expect(screen.getByRole('button', { name: 'Dashboard + 灵动岛' })).toBeInTheDocument()
    expect(screen.getByRole('switch', { name: '允许远程只读访问' })).toBeInTheDocument()
    expect(await screen.findByText('生成配对码')).toBeInTheDocument()
  })
})

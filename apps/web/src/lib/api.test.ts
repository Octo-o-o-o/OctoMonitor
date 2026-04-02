import { beforeEach, describe, expect, it, vi } from 'vitest'

function resetWindow(protocol: string, host: string) {
  Object.defineProperty(window, 'location', {
    configurable: true,
    value: {
      protocol,
      host,
    },
  })
}

describe('api helpers', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    resetWindow('http:', '127.0.0.1:4173')
  })

  it('uses same-origin paths in the browser', async () => {
    const { apiFetch, buildApiUrl, buildWsUrl } = await import('./api')

    expect(buildApiUrl('/api/health')).toBe('/api/health')
    expect(buildWsUrl('/api/stream')).toBe('ws://127.0.0.1:4173/api/stream')
    expect(apiFetch).toBeTypeOf('function')
  })

  it('uses localhost server addresses in tauri', async () => {
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {},
    })

    vi.resetModules()
    const { buildApiUrl, buildWsUrl } = await import('./api')

    expect(buildApiUrl('/api/health')).toBe('http://127.0.0.1:46321/api/health')
    expect(buildWsUrl('/api/stream')).toBe('ws://127.0.0.1:46321/api/stream')

    Reflect.deleteProperty(window, '__TAURI_INTERNALS__')
  })

  it('normalizes legacy bootstrap payloads that do not include commits yet', async () => {
    const { normalizeBootstrapPayload } = await import('./api')

    const normalized = normalizeBootstrapPayload({
      generatedAt: '2026-04-02T00:00:00.000Z',
      runs: [],
      attentions: [],
      usageBuckets: [],
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
    })

    expect(normalized.commits).toEqual([])
    expect(normalized.runs).toEqual([])
    expect(normalized.config.listenPort).toBe(46321)
  })
})

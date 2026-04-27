import type { BootstrapPayload } from './types'
import { isTauriEnvironment } from './runtimeEnvironment'

const DEFAULT_CONFIG: BootstrapPayload['config'] = {
  listenHost: '127.0.0.1',
  listenPort: 46321,
  historyDays: 30,
  companionEnabled: false,
  localIp: null,
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value != null && typeof value === 'object' ? value as Record<string, unknown> : null
}

function asArray<T>(value: unknown): T[] {
  return Array.isArray(value) ? value as T[] : []
}

export function normalizeBootstrapPayload(payload: unknown): BootstrapPayload {
  const data = asRecord(payload)
  const config = asRecord(data?.config)

  return {
    generatedAt: typeof data?.generatedAt === 'string' ? data.generatedAt : new Date().toISOString(),
    runs: asArray(data?.runs),
    attentions: asArray(data?.attentions),
    usageBuckets: asArray(data?.usageBuckets),
    commits: asArray(data?.commits),
    identities: asArray(data?.identities),
    adapterHealth: asArray(data?.adapterHealth),
    recentCompletions: asArray(data?.recentCompletions),
    pendingCrons: asArray(data?.pendingCrons),
    config: {
      listenHost: typeof config?.listenHost === 'string' ? config.listenHost : DEFAULT_CONFIG.listenHost,
      listenPort: typeof config?.listenPort === 'number' ? config.listenPort : DEFAULT_CONFIG.listenPort,
      historyDays: typeof config?.historyDays === 'number' ? config.historyDays : DEFAULT_CONFIG.historyDays,
      companionEnabled: typeof config?.companionEnabled === 'boolean' ? config.companionEnabled : DEFAULT_CONFIG.companionEnabled,
      localIp: typeof config?.localIp === 'string' ? config.localIp : null,
    },
  }
}

// Local admin surface in Tauri runs on a fixed loopback port; web/companion
// flows use same-origin URLs so reverse-proxy and remote-viewer setups work.
export function buildApiUrl(path: string): string {
  return isTauriEnvironment() ? `http://127.0.0.1:46321${path}` : path
}

export function buildWsUrl(path: string): string {
  if (isTauriEnvironment()) return `ws://127.0.0.1:46321${path}`
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  return `${proto}//${window.location.host}${path}`
}

export function apiFetch(input: string, init?: RequestInit): Promise<Response> {
  return fetch(buildApiUrl(input), init)
}

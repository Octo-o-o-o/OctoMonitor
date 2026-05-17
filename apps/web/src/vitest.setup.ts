import '@testing-library/jest-dom'
import { beforeEach, vi } from 'vitest'

function createStorageMock(): Storage {
  const store = new Map<string, string>()
  return {
    getItem: (key) => store.get(key) ?? null,
    setItem: (key, value) => {
      store.set(key, String(value))
    },
    removeItem: (key) => {
      store.delete(key)
    },
    clear: () => {
      store.clear()
    },
    key: (index) => Array.from(store.keys())[index] ?? null,
    get length() {
      return store.size
    },
  }
}

if (
  typeof globalThis.localStorage === 'undefined'
  || typeof globalThis.localStorage.getItem !== 'function'
  || typeof globalThis.localStorage.clear !== 'function'
) {
  Object.defineProperty(globalThis, 'localStorage', {
    value: createStorageMock(),
    configurable: true,
  })
}

// jsdom doesn't provide WebSocket; stub it for unit tests
if (typeof globalThis.WebSocket === 'undefined') {
  globalThis.WebSocket = class MockWebSocket {
    onopen: (() => void) | null = null
    onclose: (() => void) | null = null
    onmessage: ((ev: { data: string }) => void) | null = null
    onerror: (() => void) | null = null
    close() {}
    send() {}
  } as unknown as typeof WebSocket
}

// jsdom routes `fetch` to the real network. Without a stub, tests that mount
// components which call `apiFetch` on mount (e.g. SetupSection,
// RemoteAccessSection) trigger ECONNREFUSED against 127.0.0.1:46321 and leak
// unhandled rejections that mask real failures. Default every test to a 503
// reply so any unmocked call is loudly wrong (no `ok` branch, fresh Response
// every call so `.json()` can never double-read), and let individual tests
// override via `vi.spyOn(globalThis, 'fetch').mockImplementation(...)`.
beforeEach(() => {
  vi.spyOn(globalThis, 'fetch').mockImplementation(async () =>
    new Response('null', {
      status: 503,
      headers: { 'content-type': 'application/json' },
    }),
  )
})

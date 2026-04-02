import '@testing-library/jest-dom'

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

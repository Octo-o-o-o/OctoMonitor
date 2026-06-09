import { beforeEach, describe, expect, it, vi } from 'vitest'
import { openExternalUrl } from './openExternal'

describe('openExternalUrl', () => {
  const invoke = vi.fn<(_: string, __?: Record<string, unknown>) => Promise<unknown>>()

  beforeEach(() => {
    vi.restoreAllMocks()
    invoke.mockReset()
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke },
    })
  })

  it('opens codex urls through the desktop invoke bridge', async () => {
    invoke.mockResolvedValue(undefined)

    await openExternalUrl('codex://threads/abc')

    expect(invoke).toHaveBeenCalledWith('open_external', { url: 'codex://threads/abc' })
  })

  it('rejects unsupported schemes before invoking desktop code', async () => {
    await expect(openExternalUrl('https://example.com')).rejects.toThrow('Unsupported URL scheme')
    expect(invoke).not.toHaveBeenCalled()
  })
})

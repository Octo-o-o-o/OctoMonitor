import { beforeEach, describe, expect, it, vi } from 'vitest'
import {
  applyDesktopZoom,
  bootstrapDesktopWebview,
  clampDesktopZoom,
  loadDesktopZoom,
  nextDesktopZoom,
  saveDesktopZoom,
} from './desktopZoom'

describe('desktop zoom helpers', () => {
  const invoke = vi.fn<(_: string, __?: Record<string, unknown>) => Promise<unknown>>()

  beforeEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
    invoke.mockReset()
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke },
    })
  })

  it('clamps, persists, and applies native webview zoom values', async () => {
    invoke.mockResolvedValue(undefined)

    saveDesktopZoom(5)
    expect(loadDesktopZoom()).toBe(3)

    await applyDesktopZoom(0.25)
    expect(invoke).toHaveBeenCalledWith('plugin:webview|set_webview_zoom', { value: 0.5 })
  })

  it('bootstraps auto-resize and the saved zoom value in tauri', async () => {
    invoke.mockResolvedValue(undefined)

    saveDesktopZoom(1.4)
    bootstrapDesktopWebview()
    await Promise.resolve()
    await Promise.resolve()

    expect(invoke).toHaveBeenCalledWith('plugin:webview|set_webview_auto_resize', { value: true })
    expect(invoke).toHaveBeenCalledWith('plugin:webview|set_webview_zoom', { value: 1.4 })
  })

  it('moves zoom in 10 percent steps and resets to default', () => {
    expect(clampDesktopZoom(Number.NaN)).toBe(1)
    expect(nextDesktopZoom(1, 'in')).toBe(1.1)
    expect(nextDesktopZoom(1, 'out')).toBe(0.9)
    expect(nextDesktopZoom(1.9, 'reset')).toBe(1)
  })
})

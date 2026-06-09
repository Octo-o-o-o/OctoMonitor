import { afterEach, describe, expect, it, vi } from 'vitest'
import { applyDesktopDisplaySettings } from './desktopDisplay'

describe('desktopDisplay', () => {
  afterEach(() => {
    delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
    vi.restoreAllMocks()
  })

  it('does nothing outside the desktop app', async () => {
    await expect(applyDesktopDisplaySettings({ mode: 'both', position: 'auto' })).resolves.toBeUndefined()
  })

  it('invokes the desktop display command inside Tauri', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined)
    ;(window as Window & {
      __TAURI_INTERNALS__?: { invoke: typeof invoke }
    }).__TAURI_INTERNALS__ = { invoke }

    await applyDesktopDisplaySettings({ mode: 'island', position: 'topCenter' })

    expect(invoke).toHaveBeenCalledWith('apply_display_mode', {
      mode: 'island',
      position: 'topCenter',
    })
  })
})

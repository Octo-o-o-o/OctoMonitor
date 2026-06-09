import { isTauriEnvironment } from './runtimeEnvironment'
import { STORAGE_KEYS } from './storageKeys'

const DEFAULT_ZOOM = 1
const MIN_ZOOM = 0.5
const MAX_ZOOM = 3
const ZOOM_STEP = 0.1

export type DesktopZoomAction = 'in' | 'out' | 'reset'

type TauriInternalsWindow = Window & {
  __TAURI_INTERNALS__?: {
    invoke?: (command: string, payload?: Record<string, unknown>) => Promise<unknown>
  }
}

function roundZoom(value: number): number {
  return Math.round(value * 100) / 100
}

export function clampDesktopZoom(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_ZOOM
  return roundZoom(Math.min(Math.max(value, MIN_ZOOM), MAX_ZOOM))
}

export function loadDesktopZoom(): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.desktopZoom)
    if (!raw) return DEFAULT_ZOOM
    return clampDesktopZoom(Number(raw))
  } catch {
    return DEFAULT_ZOOM
  }
}

export function saveDesktopZoom(value: number) {
  try {
    localStorage.setItem(STORAGE_KEYS.desktopZoom, String(clampDesktopZoom(value)))
  } catch (err) {
    console.warn('[OctoMonitor] storage.write.desktopZoom', err)
  }
}

export function getTauriInvoke() {
  if (typeof window === 'undefined') return null
  return (window as TauriInternalsWindow).__TAURI_INTERNALS__?.invoke ?? null
}

async function setDesktopAutoResize(enabled: boolean) {
  const invoke = getTauriInvoke()
  if (!invoke) return

  await invoke('plugin:webview|set_webview_auto_resize', {
    value: enabled,
  })
}

export async function applyDesktopZoom(value: number) {
  const invoke = getTauriInvoke()
  if (!invoke) return

  await invoke('plugin:webview|set_webview_zoom', {
    value: clampDesktopZoom(value),
  })
}

export function bootstrapDesktopWebview() {
  if (!isTauriEnvironment()) return
  void (async () => {
    await Promise.allSettled([
      setDesktopAutoResize(true),
      applyDesktopZoom(loadDesktopZoom()),
    ])
  })()
}

export function nextDesktopZoom(current: number, action: DesktopZoomAction): number {
  if (action === 'reset') return DEFAULT_ZOOM
  if (action === 'in') return clampDesktopZoom(current + ZOOM_STEP)
  return clampDesktopZoom(current - ZOOM_STEP)
}

import { getTauriInvoke } from './desktopZoom'
import { isTauriEnvironment } from './runtimeEnvironment'

const ALLOWED_SCHEMES = new Set(['codex:'])

export async function openExternalUrl(url: string): Promise<void> {
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    throw new Error('Invalid URL')
  }

  if (!ALLOWED_SCHEMES.has(parsed.protocol)) {
    throw new Error('Unsupported URL scheme')
  }

  if (!isTauriEnvironment()) {
    throw new Error('External opening is only available in the desktop app')
  }

  const invoke = getTauriInvoke()
  if (!invoke) {
    throw new Error('Desktop invoke API is unavailable')
  }

  await invoke('open_external', { url })
}

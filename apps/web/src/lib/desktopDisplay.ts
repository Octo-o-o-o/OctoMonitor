import { getTauriInvoke } from './desktopZoom'
import type { DesktopDisplayMode, IslandPosition } from './preferences'
import { isTauriEnvironment } from './runtimeEnvironment'

export type DesktopDisplaySettings = {
  mode: DesktopDisplayMode
  position: IslandPosition
}

export async function applyDesktopDisplaySettings(settings: DesktopDisplaySettings): Promise<void> {
  if (!isTauriEnvironment()) return
  const invoke = getTauriInvoke()
  if (!invoke) return
  await invoke('apply_display_mode', settings)
}

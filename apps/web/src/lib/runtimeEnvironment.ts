export function isTauriEnvironment(): boolean {
  if (typeof window === 'undefined') return false
  return '__TAURI_INTERNALS__' in window
    || '__TAURI__' in window
    || window.location.protocol === 'tauri:'
}

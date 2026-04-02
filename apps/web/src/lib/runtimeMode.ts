export type RuntimeMode = 'local' | 'remoteViewer'

export function getRuntimeMode(): RuntimeMode {
  if (typeof window === 'undefined') return 'local'
  return window.location.port === '46322' ? 'remoteViewer' : 'local'
}

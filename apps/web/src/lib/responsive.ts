import { useSyncExternalStore } from 'react'

export const NARROW_LAYOUT_QUERY = '(max-width: 768px)'

function getMediaQuerySnapshot(query: string, fallback: boolean): boolean {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return fallback
  }
  return window.matchMedia(query).matches
}

export function useMediaQuery(query: string, fallback = false): boolean {
  return useSyncExternalStore(
    (onStoreChange) => {
      if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
        return () => {}
      }

      const mediaQuery = window.matchMedia(query)
      const handler = () => onStoreChange()

      if (typeof mediaQuery.addEventListener === 'function') {
        mediaQuery.addEventListener('change', handler)
        return () => mediaQuery.removeEventListener('change', handler)
      }

      mediaQuery.addListener(handler)
      return () => mediaQuery.removeListener(handler)
    },
    () => getMediaQuerySnapshot(query, fallback),
    () => fallback,
  )
}

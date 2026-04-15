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
      mediaQuery.addEventListener('change', onStoreChange)
      return () => mediaQuery.removeEventListener('change', onStoreChange)
    },
    () => getMediaQuerySnapshot(query, fallback),
    () => fallback,
  )
}

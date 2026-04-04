import { endOfDay, startOfDay } from './dateRange'
import type { SnapshotWindow } from './preferences'

export type SnapshotRange = {
  from: Date
  to: Date
}

const snapshotWindowDays: Record<Exclude<SnapshotWindow, 'all'>, number> = {
  day: 1,
  week: 7,
  month: 30,
}

export function buildSnapshotRange(
  window: SnapshotWindow,
  loadedRange: SnapshotRange | null,
): SnapshotRange | null {
  if (!loadedRange) {
    return null
  }

  const loadedFrom = startOfDay(loadedRange.from)
  const loadedTo = endOfDay(loadedRange.to)

  if (window === 'all') {
    return { from: loadedFrom, to: loadedTo }
  }

  const requestedTo = endOfDay(new Date())
  const requestedFrom = startOfDay(new Date())
  requestedFrom.setDate(requestedFrom.getDate() - snapshotWindowDays[window] + 1)

  const effectiveTo = new Date(Math.min(loadedTo.getTime(), requestedTo.getTime()))
  const effectiveFrom = new Date(Math.max(loadedFrom.getTime(), requestedFrom.getTime()))

  if (effectiveFrom > effectiveTo) {
    return { from: loadedFrom, to: loadedTo }
  }

  return { from: effectiveFrom, to: effectiveTo }
}

export function isSnapshotWindowClamped(window: SnapshotWindow, loadedDays: number): boolean {
  if (window === 'all') {
    return false
  }
  return loadedDays < snapshotWindowDays[window]
}

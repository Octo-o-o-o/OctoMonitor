import { apiFetch } from './api'
import type { CommitHistoryPayload, UsageHistoryPayload } from './types'

export type HistorySelection = {
  from: Date
  to: Date
}

export type DataMode = 'snapshot' | 'history'

export function createHistorySelection(days: number): HistorySelection {
  const to = new Date()
  to.setHours(23, 59, 59, 999)

  const from = new Date()
  from.setHours(0, 0, 0, 0)
  from.setDate(from.getDate() - days + 1)

  return { from, to }
}

function buildHistoryQuery(range: HistorySelection): string {
  const params = new URLSearchParams({
    from: range.from.toISOString(),
    to: range.to.toISOString(),
  })
  return params.toString()
}

export async function fetchUsageHistory(
  range: HistorySelection,
  signal?: AbortSignal,
): Promise<UsageHistoryPayload> {
  const response = await apiFetch(`/api/history/usage?${buildHistoryQuery(range)}`, { signal })
  if (!response.ok) {
    throw new Error('usage history request failed')
  }
  return await response.json() as UsageHistoryPayload
}

export async function fetchCommitHistory(
  range: HistorySelection,
  signal?: AbortSignal,
): Promise<CommitHistoryPayload> {
  const response = await apiFetch(`/api/history/commits?${buildHistoryQuery(range)}`, { signal })
  if (!response.ok) {
    throw new Error('commit history request failed')
  }
  return await response.json() as CommitHistoryPayload
}

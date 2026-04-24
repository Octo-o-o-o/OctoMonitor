import type { RunRecord } from './types'
import type { MonitorQuickFilter } from '../store/monitorStore'

const attentionStates = new Set([
  'waitingApproval',
  'error',
  'gatewayOffline',
  'limitExceeded',
  'contextExceeded',
])

export function runMatchesQuickFilter(run: RunRecord, filter: MonitorQuickFilter): boolean {
  switch (filter) {
    case 'all':
      return true
    case 'active':
      return run.state === 'active'
    case 'attention':
      return attentionStates.has(run.state)
  }
}

export function runMatchesSearch(run: RunRecord, searchRaw: string): boolean {
  const query = searchRaw.trim().toLowerCase()
  if (!query) return true
  const haystack = [
    run.projectName,
    run.workspaceShort,
    run.workspacePath,
    run.lastQuestion ?? '',
    run.firstQuestion ?? '',
    run.lastAction ?? '',
    run.id,
  ]
    .join(' ')
    .toLowerCase()
  return haystack.includes(query)
}

export function applyMonitorFilters(
  runs: RunRecord[],
  filter: MonitorQuickFilter,
  search: string,
): RunRecord[] {
  if (filter === 'all' && search.trim() === '') return runs
  return runs.filter(
    (run) => runMatchesQuickFilter(run, filter) && runMatchesSearch(run, search),
  )
}

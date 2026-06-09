import type { RunRecord } from './types'

export type RunOpenAffordance = 'openCodex' | 'inspectOnly'

export function getRunOpenAffordance(run: Pick<RunRecord, 'tool' | 'threadId'>): RunOpenAffordance {
  return run.tool === 'codex' && typeof run.threadId === 'string' && run.threadId.trim() !== ''
    ? 'openCodex'
    : 'inspectOnly'
}

export function buildCodexDeepLink(threadId: string): string {
  return `codex://threads/${encodeURIComponent(threadId)}`
}

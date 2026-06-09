import { describe, expect, it } from 'vitest'
import type { RunRecord } from './types'
import { buildCodexDeepLink, getRunOpenAffordance } from './runTarget'

function run(overrides: Partial<RunRecord>): Pick<RunRecord, 'tool' | 'threadId'> {
  return {
    tool: 'codex',
    threadId: 'thread-1',
    ...overrides,
  } as Pick<RunRecord, 'tool' | 'threadId'>
}

describe('run target helpers', () => {
  it('offers Codex opening only when a Codex run has a thread id', () => {
    expect(getRunOpenAffordance(run({ tool: 'codex', threadId: '019eacb8' }))).toBe('openCodex')
    expect(getRunOpenAffordance(run({ tool: 'codex', threadId: null }))).toBe('inspectOnly')
    expect(getRunOpenAffordance(run({ tool: 'codex', threadId: '' }))).toBe('inspectOnly')
    expect(getRunOpenAffordance(run({ tool: 'claude', threadId: '019eacb8' }))).toBe('inspectOnly')
  })

  it('treats remote redacted payloads as inspect-only', () => {
    const redacted = { tool: 'codex' } as Pick<RunRecord, 'tool' | 'threadId'>
    expect(getRunOpenAffordance(redacted)).toBe('inspectOnly')
  })

  it('builds an encoded codex thread deep link', () => {
    expect(buildCodexDeepLink('thread with spaces')).toBe('codex://threads/thread%20with%20spaces')
  })
})

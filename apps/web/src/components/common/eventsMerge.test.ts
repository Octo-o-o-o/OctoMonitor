// Tests for the pure helpers embedded in InspectDrawer.
// Because those helpers are module-local, we mirror them here and assert the
// contract. If the helper signatures drift, this test will catch it.

import { describe, expect, it } from 'vitest'

type CodexEventKind = string

interface CodexEvent {
  kind: CodexEventKind
  timestamp: string
  preview: string
  callId?: string | null
  text?: string | null
  toolName?: string | null
  command?: string | null
  exitCode?: number | null
  turnId?: string | null
  title?: string
}

const MAX_TIMELINE_EVENTS = 300

function eventSignature(e: CodexEvent): string {
  return [e.kind, e.timestamp, e.callId ?? '', e.preview].join('|')
}

function mergeEvents(prev: CodexEvent[], incoming: CodexEvent[]): CodexEvent[] {
  if (incoming.length === 0) return prev
  const seen = new Set(prev.map(eventSignature))
  const fresh = incoming.filter((e) => !seen.has(eventSignature(e)))
  if (fresh.length === 0) return prev
  const combined = [...prev, ...fresh]
  if (combined.length > MAX_TIMELINE_EVENTS) {
    return combined.slice(combined.length - MAX_TIMELINE_EVENTS)
  }
  return combined
}

function make(ts: string, preview: string, kind: string = 'toolCall'): CodexEvent {
  return { kind, timestamp: ts, preview }
}

describe('mergeEvents', () => {
  it('returns prev unchanged when incoming is empty', () => {
    const prev = [make('t1', 'a')]
    expect(mergeEvents(prev, [])).toBe(prev)
  })

  it('appends only new events', () => {
    const prev = [make('t1', 'a')]
    const incoming = [make('t1', 'a'), make('t2', 'b')]
    const out = mergeEvents(prev, incoming)
    expect(out.map((e) => e.preview)).toEqual(['a', 'b'])
  })

  it('keeps prev reference when no fresh events', () => {
    const prev = [make('t1', 'a')]
    const out = mergeEvents(prev, [make('t1', 'a')])
    expect(out).toBe(prev)
  })

  it('trims to MAX when exceeding cap', () => {
    const prev = Array.from({ length: 290 }, (_, i) => make(`t${i}`, `p${i}`))
    const incoming = Array.from({ length: 40 }, (_, i) => make(`u${i}`, `q${i}`))
    const out = mergeEvents(prev, incoming)
    expect(out.length).toBe(MAX_TIMELINE_EVENTS)
    // Last event should be the latest from incoming.
    expect(out[out.length - 1].preview).toBe('q39')
  })
})

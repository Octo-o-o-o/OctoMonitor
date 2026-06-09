import { useEffect, useMemo, useRef, useState } from 'react'
import { useMonitorStore, selectSelectedRun } from '../store/monitorStore'
import { useI18n } from '../lib/i18n'
import { stateLabelKeys } from '../lib/i18nMaps'
import { formatTokens, formatCost, formatDuration, formatDateTime } from '../lib/format'
import { buildUsageBucketIndex } from '../lib/usage'
import { apiFetch } from '../lib/api'
import { getRuntimeMode } from '../lib/runtimeMode'
import { isTauriEnvironment } from '../lib/runtimeEnvironment'
import { buildCodexDeepLink, getRunOpenAffordance } from '../lib/runTarget'
import { openExternalUrl } from '../lib/openExternal'
import { CopyButton } from './common/CopyButton'
import { useToastStore } from '../store/toastStore'

type InspectEntry = {
  kind: 'input' | 'output'
  timestamp: string
  text: string
}

interface ResumeCommandPayload {
  command: string | null
  tool: string
  note: string | null
}

type CodexEventKind =
  | 'userMessage'
  | 'assistantMessage'
  | 'reasoning'
  | 'toolCall'
  | 'toolOutput'
  | 'webSearch'
  | 'tokenCount'
  | 'taskStarted'
  | 'taskComplete'
  | 'taskAborted'
  | 'turnAborted'
  | 'context'

interface CodexEvent {
  kind: CodexEventKind
  timestamp: string
  turnId?: string | null
  title: string
  preview: string
  text?: string | null
  toolName?: string | null
  command?: string | null
  exitCode?: number | null
  callId?: string | null
}

interface EventsPayload {
  tool: string
  events: CodexEvent[]
  cursor: number
  reset: boolean
}

const MAX_TIMELINE_EVENTS = 300
const EVENT_POLL_MS = 2000

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

const errorStates = new Set(['error', 'gatewayOffline', 'limitExceeded', 'contextExceeded'])

const stateClassMap: Record<string, string> = {
  active: 'inspect-state--active',
  waitingApproval: 'inspect-state--waiting',
}

const toolColorMap: Record<string, string> = {
  claude: 'var(--claude-accent)',
  codex: 'var(--codex-accent)',
  openClaw: 'var(--openclaw-accent)',
  hermes: 'var(--hermes-accent)',
}

export function InspectDrawer() {
  const selectedRun = useMonitorStore(selectSelectedRun)
  const selectedRunId = useMonitorStore((s) => s.selectedRunId)
  const usageBuckets = useMonitorStore((s) => s.data?.usageBuckets)
  const selectRun = useMonitorStore((s) => s.selectRun)
  const markRunVisited = useMonitorStore((s) => s.markRunVisited)
  const pushToast = useToastStore((s) => s.pushToast)
  const { t } = useI18n()
  const [entries, setEntries] = useState<InspectEntry[]>([])
  const [entriesLoading, setEntriesLoading] = useState(false)
  const [resumeCommand, setResumeCommand] = useState<ResumeCommandPayload | null>(null)
  const [events, setEvents] = useState<CodexEvent[]>([])
  const [eventsLoading, setEventsLoading] = useState(false)
  const [openingCodex, setOpeningCodex] = useState(false)
  const [expandedEventIds, setExpandedEventIds] = useState<Set<string>>(() => new Set())
  const cursorRef = useRef<number | undefined>(undefined)
  const timelineRef = useRef<HTMLDivElement | null>(null)
  const nearBottomRef = useRef(true)
  const runtimeMode = getRuntimeMode()
  const canUseDesktopOpen = runtimeMode === 'local' && isTauriEnvironment()

  const useCodexEvents = runtimeMode === 'local' && selectedRun?.tool === 'codex'
  const openAffordance = selectedRun ? getRunOpenAffordance(selectedRun) : 'inspectOnly'
  const showOpenInCodex = Boolean(
    selectedRun && canUseDesktopOpen && openAffordance === 'openCodex',
  )
  const usageBucketIndex = useMemo(
    () => buildUsageBucketIndex(usageBuckets ?? []),
    [usageBuckets],
  )
  const selectedRunCost = selectedRunId == null
    ? undefined
    : usageBucketIndex.get(selectedRunId)?.costUsd

  useEffect(() => {
    if (!selectedRun) return
    function onKeyDown(e: KeyboardEvent) {
      if (e.key === 'Escape') selectRun(undefined)
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [selectedRun, selectRun])

  useEffect(() => {
    let cancelled = false
    setResumeCommand(null)

    if (!selectedRun || runtimeMode === 'remoteViewer') {
      return () => { cancelled = true }
    }

    void (async () => {
      try {
        const response = await apiFetch(
          `/api/runs/${encodeURIComponent(selectedRun.id)}/resume-command`,
        )
        if (!response.ok) return
        const payload = await response.json() as ResumeCommandPayload
        if (!cancelled) setResumeCommand(payload)
      } catch {
        // best-effort; resume command is advisory only
      }
    })()

    return () => { cancelled = true }
  }, [runtimeMode, selectedRun])

  // Legacy inspect fetch for non-Codex runs (Claude / OpenClaw).
  useEffect(() => {
    let cancelled = false
    setEntries([])
    setEntriesLoading(false)

    if (!selectedRun || runtimeMode === 'remoteViewer' || useCodexEvents) {
      return () => { cancelled = true }
    }

    setEntriesLoading(true)
    void (async () => {
      try {
        const response = await apiFetch(`/api/runs/${encodeURIComponent(selectedRun.id)}/inspect`)
        if (!response.ok) throw new Error(`inspect fetch failed: ${response.status}`)
        const payload = await response.json() as { entries?: InspectEntry[] }
        if (!cancelled) setEntries(Array.isArray(payload.entries) ? payload.entries : [])
      } catch {
        if (!cancelled) setEntries([])
      } finally {
        if (!cancelled) setEntriesLoading(false)
      }
    })()

    return () => { cancelled = true }
  }, [runtimeMode, selectedRun, useCodexEvents])

  // Codex events fetch + polling loop.
  useEffect(() => {
    if (!selectedRun || !useCodexEvents) {
      setEvents([])
      cursorRef.current = undefined
      return () => { /* no cleanup */ }
    }

    const runId = selectedRun.id
    let cancelled = false
    let timer: ReturnType<typeof setTimeout> | undefined
    setEvents([])
    setEventsLoading(true)
    cursorRef.current = undefined

    async function fetchOnce(): Promise<void> {
      const cursor = cursorRef.current
      const url = cursor === undefined
        ? `/api/runs/${encodeURIComponent(runId)}/events?limit=${MAX_TIMELINE_EVENTS}`
        : `/api/runs/${encodeURIComponent(runId)}/events?cursor=${cursor}&limit=${MAX_TIMELINE_EVENTS}`
      try {
        const response = await apiFetch(url)
        if (!response.ok) return
        const payload = await response.json() as EventsPayload
        if (cancelled) return
        if (payload.reset) {
          cursorRef.current = payload.cursor
          setEvents(payload.events.slice(-MAX_TIMELINE_EVENTS))
          return
        }
        cursorRef.current = payload.cursor
        setEvents((prev) => mergeEvents(prev, payload.events))
      } catch {
        // best-effort; the next poll will retry
      }
    }

    function schedule() {
      if (cancelled) return
      if (document.hidden) return
      if (selectedRun?.state !== 'active') return
      timer = setTimeout(tick, EVENT_POLL_MS)
    }

    async function tick() {
      if (cancelled) return
      await fetchOnce()
      if (!cancelled) schedule()
    }

    function onVisibility() {
      if (cancelled) return
      if (!document.hidden && selectedRun?.state === 'active') {
        void (async () => {
          await fetchOnce()
          schedule()
        })()
      }
    }

    void (async () => {
      await fetchOnce()
      if (!cancelled) setEventsLoading(false)
      schedule()
    })()

    document.addEventListener('visibilitychange', onVisibility)

    return () => {
      cancelled = true
      if (timer) clearTimeout(timer)
      document.removeEventListener('visibilitychange', onVisibility)
    }
  }, [selectedRun, useCodexEvents])

  // Near-bottom scroll lock: stay pinned to the newest event unless the
  // user has scrolled upward.
  useEffect(() => {
    const el = timelineRef.current
    if (!el) return
    if (nearBottomRef.current) {
      el.scrollTop = el.scrollHeight
    }
  }, [events])

  function handleTimelineScroll() {
    const el = timelineRef.current
    if (!el) return
    const distance = el.scrollHeight - el.scrollTop - el.clientHeight
    nearBottomRef.current = distance < 40
  }

  function toggleEventExpansion(id: string) {
    setExpandedEventIds((prev) => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  if (!selectedRun) return null

  async function handleOpenInCodex() {
    if (!selectedRun?.threadId) return
    setOpeningCodex(true)
    try {
      await openExternalUrl(buildCodexDeepLink(selectedRun.threadId))
    } catch {
      pushToast({ kind: 'error', message: t('drawer.openInCodex.error') })
    } finally {
      markRunVisited(selectedRun.id)
      setOpeningCodex(false)
    }
  }

  const isError = errorStates.has(selectedRun.state)
  const stateClass = stateClassMap[selectedRun.state]
    ?? (isError ? 'inspect-state--error' : 'inspect-state--done')
  const toolColor = toolColorMap[selectedRun.tool] ?? 'var(--openclaw-accent)'

  return (
    <div className="inspect-overlay" onClick={(e) => {
      if (e.target === e.currentTarget) selectRun(undefined)
    }}>
      <div className="inspect-panel" role="dialog" aria-modal="true" aria-labelledby="inspect-title">
        <div className="inspect-header">
          <div className="inspect-title-row">
            <div className="inspect-title-left">
              <span className="inspect-tool-dot" style={{ background: toolColor }} />
              <h3 id="inspect-title" className="inspect-project">{selectedRun.projectName}</h3>
              <span className="inspect-path">{selectedRun.workspaceShort || ''}</span>
            </div>
            <span className={`inspect-state ${stateClass}`}>{t(stateLabelKeys[selectedRun.state])}</span>
          </div>
          <div className="inspect-time-row">
            <span className="inspect-time-item">
              <span className="inspect-time-label">{t('drawer.started')}</span>
              <span className="inspect-time-value">{formatDateTime(selectedRun.startedAt)}</span>
            </span>
            <span className="inspect-time-sep" />
            <span className="inspect-time-item">
              <span className="inspect-time-label">{t('drawer.updated')}</span>
              <span className="inspect-time-value">{formatDateTime(selectedRun.lastActivityAt)}</span>
            </span>
          </div>
        </div>

        <div className="inspect-body">
          <div className="inspect-grid">
            <div className="inspect-cell">
              <span className="inspect-cell-label">{t('drawer.model')}</span>
              <span className="inspect-cell-value">{selectedRun.model ?? '—'}</span>
            </div>
            {selectedRun.tool === 'openClaw' && selectedRun.provider && (
              <div className="inspect-cell">
                <span className="inspect-cell-label">{t('drawer.provider')}</span>
                <span className="inspect-cell-value">{selectedRun.provider}</span>
              </div>
            )}
            <div className="inspect-cell">
              <span className="inspect-cell-label">{t('drawer.duration')}</span>
              <span className="inspect-cell-value">{formatDuration(selectedRun.elapsedMs)}</span>
            </div>
            <div className="inspect-cell">
              <span className="inspect-cell-label">{t('drawer.messages')}</span>
              <span className="inspect-cell-value">{selectedRun.messageCount}</span>
            </div>
            <div className="inspect-cell">
              <span className="inspect-cell-label">{t('drawer.tokens')}</span>
              <span className="inspect-cell-value">{formatTokens(selectedRun.tokens.total)}</span>
            </div>
            <div className="inspect-cell">
              <span className="inspect-cell-label">{t('drawer.cost')}</span>
              <span className="inspect-cell-value">{formatCost(selectedRunCost ?? selectedRun.cost.usd)}</span>
            </div>
          </div>

          {selectedRun.originLabel && (
            <div className="inspect-section">
              <span className="inspect-section-label">{t('drawer.origin')}</span>
              <span className="inspect-section-value">{selectedRun.originLabel}</span>
            </div>
          )}

          {runtimeMode === 'local' && (
            <div className="inspect-section">
              {showOpenInCodex && (
                <div className="inspect-open-codex">
                  <button
                    type="button"
                    className="inspect-open-codex-button"
                    onClick={handleOpenInCodex}
                    disabled={openingCodex}
                  >
                    {t('drawer.openInCodex')}
                  </button>
                  <span className="inspect-open-codex-hint">
                    {t('drawer.openInCodex.hint')}
                  </span>
                </div>
              )}
              <div className="inspect-copy-row">
                <CopyButton text={selectedRun.id} ariaLabel={t('drawer.copy.runId')} />
                <span className="inspect-section-value">{selectedRun.id}</span>
              </div>
              {selectedRun.threadId && (
                <div className="inspect-copy-row">
                  <CopyButton
                    text={selectedRun.threadId}
                    ariaLabel={t('drawer.copy.threadId')}
                  />
                  <span className="inspect-section-value">{selectedRun.threadId}</span>
                </div>
              )}
              {selectedRun.workspacePath && (
                <div className="inspect-copy-row">
                  <CopyButton
                    text={selectedRun.workspacePath}
                    ariaLabel={t('drawer.copy.workspacePath')}
                  />
                  <span className="inspect-section-value">{selectedRun.workspacePath}</span>
                </div>
              )}
              {selectedRun.transcriptPath && (
                <div className="inspect-copy-row">
                  <CopyButton
                    text={selectedRun.transcriptPath}
                    ariaLabel={t('drawer.copy.transcriptPath')}
                  />
                  <span className="inspect-section-value">{selectedRun.transcriptPath}</span>
                </div>
              )}
              {resumeCommand?.command ? (
                <div className="inspect-copy-row">
                  <CopyButton
                    text={resumeCommand.command}
                    ariaLabel={t('drawer.copy.resumeCommand')}
                  />
                  <span className="inspect-section-label">
                    {t('drawer.resumeCommand.label')}
                  </span>
                  <code className="inspect-resume-cmd">{resumeCommand.command}</code>
                </div>
              ) : resumeCommand && resumeCommand.note ? (
                <div className="inspect-copy-row">
                  <span className="inspect-resume-unavailable">
                    {t('drawer.resumeCommand.unavailable')}
                  </span>
                </div>
              ) : null}
            </div>
          )}

          {selectedRun.firstQuestion && (
            <div className="inspect-section">
              <span className="inspect-section-label">{t('drawer.firstQuestion')}</span>
              <p className="inspect-question">{selectedRun.firstQuestion}</p>
            </div>
          )}
          {selectedRun.lastQuestion && selectedRun.lastQuestion !== selectedRun.firstQuestion && (
            <div className="inspect-section">
              <span className="inspect-section-label">{t('drawer.lastQuestion')}</span>
              <p className="inspect-question">{selectedRun.lastQuestion}</p>
            </div>
          )}

          {useCodexEvents && (eventsLoading || events.length > 0) && (
            <div className="inspect-section">
              <span className="inspect-section-label">{t('drawer.timeline')}</span>
              <div
                className="inspect-event-list"
                ref={timelineRef}
                onScroll={handleTimelineScroll}
              >
                {eventsLoading && events.length === 0 && (
                  <div className="inspect-io-empty">{t('drawer.loadingEntries')}</div>
                )}
                {events.map((ev, index) => {
                  const id = `${ev.timestamp}-${ev.callId ?? ''}-${index}`
                  const isError = ev.exitCode != null && ev.exitCode !== 0
                  const expanded = expandedEventIds.has(id)
                  const hasLongText = Boolean(ev.text && ev.text.length > 400)
                  return (
                    <div
                      key={id}
                      className={`inspect-event inspect-event--${ev.kind}${isError ? ' inspect-event--error' : ''}`}
                    >
                      <div className="inspect-event-meta">
                        <span className="inspect-event-kind">{ev.kind}</span>
                        <span className="inspect-event-time">{formatDateTime(ev.timestamp)}</span>
                        {ev.toolName && (
                          <span className="inspect-event-tool">{ev.toolName}</span>
                        )}
                        {ev.exitCode != null && (
                          <span className={`inspect-event-exit${isError ? ' inspect-event-exit--err' : ''}`}>
                            exit {ev.exitCode}
                          </span>
                        )}
                      </div>
                      <div className="inspect-event-preview">{ev.preview}</div>
                      {ev.command && (
                        <code className="inspect-event-command">{ev.command}</code>
                      )}
                      {ev.text && (
                        <div className="inspect-event-text-wrap">
                          <div className={`inspect-event-text${!expanded && hasLongText ? ' inspect-event-text--clamped' : ''}`}>
                            {ev.text}
                          </div>
                          {hasLongText && (
                            <button
                              type="button"
                              className="inspect-event-toggle"
                              onClick={() => toggleEventExpansion(id)}
                            >
                              {expanded ? t('drawer.collapse') : t('drawer.expand')}
                            </button>
                          )}
                        </div>
                      )}
                    </div>
                  )
                })}
              </div>
            </div>
          )}

          {!useCodexEvents && runtimeMode === 'local' && (entriesLoading || entries.length > 0) && (
            <div className="inspect-section">
              <span className="inspect-section-label">{t('drawer.timeline')}</span>
              <div className="inspect-io-list">
                {entriesLoading && (
                  <div className="inspect-io-empty">{t('drawer.loadingEntries')}</div>
                )}
                {!entriesLoading && entries.map((entry, index) => (
                  <div
                    key={`${entry.kind}-${entry.timestamp}-${index}`}
                    className={`inspect-io-item inspect-io-item--${entry.kind}`}
                  >
                    <div className="inspect-io-meta">
                      <span className={`inspect-io-kind inspect-io-kind--${entry.kind}`}>
                        {t(entry.kind === 'input' ? 'drawer.input' : 'drawer.output')}
                      </span>
                      <span className="inspect-io-time">{formatDateTime(entry.timestamp)}</span>
                    </div>
                    <p className="inspect-io-text">{entry.text}</p>
                  </div>
                ))}
              </div>
            </div>
          )}

          {isError && (
            <div className="inspect-error">
              <span className="inspect-error-state">{selectedRun.state}</span>
              {(selectedRun.errorMessage ?? selectedRun.lastTail) && (
                <p className="inspect-error-msg">{selectedRun.errorMessage ?? selectedRun.lastTail}</p>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

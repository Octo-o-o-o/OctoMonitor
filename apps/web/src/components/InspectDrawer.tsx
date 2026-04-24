import { useEffect, useMemo, useState } from 'react'
import { useMonitorStore, selectSelectedRun } from '../store/monitorStore'
import { useI18n } from '../lib/i18n'
import { stateLabelKeys } from '../lib/i18nMaps'
import { formatTokens, formatCost, formatDuration, formatDateTime } from '../lib/format'
import { buildUsageBucketIndex } from '../lib/usage'
import { apiFetch } from '../lib/api'
import { getRuntimeMode } from '../lib/runtimeMode'
import { CopyButton } from './common/CopyButton'

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
  const { t } = useI18n()
  const [entries, setEntries] = useState<InspectEntry[]>([])
  const [entriesLoading, setEntriesLoading] = useState(false)
  const [resumeCommand, setResumeCommand] = useState<ResumeCommandPayload | null>(null)
  const runtimeMode = getRuntimeMode()
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

  useEffect(() => {
    let cancelled = false
    setEntries([])
    setEntriesLoading(false)

    if (!selectedRun || runtimeMode === 'remoteViewer') {
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
  }, [runtimeMode, selectedRun])

  if (!selectedRun) return null

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

          {(runtimeMode === 'local' && (entriesLoading || entries.length > 0)) && (
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

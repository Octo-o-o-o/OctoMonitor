import { useEffect, useRef, useState } from 'react'
import { useMonitorStore, selectSelectedRun } from '../store/monitorStore'
import { useI18n } from '../lib/i18n'
import { formatTokens, formatCost, formatDuration, formatDateTime } from '../lib/format'
import { buildUsageBucketIndex } from '../lib/usage'
import { apiFetch } from '../lib/api'
import { getRuntimeMode } from '../lib/runtimeMode'

type InspectEntry = {
  kind: 'input' | 'output'
  timestamp: string
  text: string
}

function stateLabel(state: string, t: (key: any) => string): string {
  return t(`state.${state}` as any) ?? state.toUpperCase()
}

function formatQuota(pct: number | undefined): string {
  if (pct == null) return '—'
  return `${Math.round(pct)}%`
}

export function InspectDrawer() {
  const selectedRun = useMonitorStore(selectSelectedRun)
  const selectedRunCost = useMonitorStore((s) => {
    const data = s.data
    if (!data || s.selectedRunId == null) return undefined
    return buildUsageBucketIndex(data.usageBuckets).get(s.selectedRunId)?.costUsd
  })
  const selectRun = useMonitorStore((s) => s.selectRun)
  const { t } = useI18n()
  const panelRef = useRef<HTMLDivElement>(null)
  const [entries, setEntries] = useState<InspectEntry[]>([])
  const [entriesLoading, setEntriesLoading] = useState(false)
  const runtimeMode = getRuntimeMode()

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
    setEntries([])

    if (!selectedRun || runtimeMode === 'remoteViewer') {
      setEntriesLoading(false)
      return () => {
        cancelled = true
      }
    }

    setEntriesLoading(true)
    void apiFetch(`/api/runs/${encodeURIComponent(selectedRun.id)}/inspect`)
      .then(async (response) => {
        if (!response.ok) {
          throw new Error(`inspect fetch failed: ${response.status}`)
        }
        return response.json() as Promise<{ entries?: InspectEntry[] }>
      })
      .then((payload) => {
        if (!cancelled) {
          setEntries(Array.isArray(payload.entries) ? payload.entries : [])
        }
      })
      .catch(() => {
        if (!cancelled) {
          setEntries([])
        }
      })
      .finally(() => {
        if (!cancelled) {
          setEntriesLoading(false)
        }
      })

    return () => {
      cancelled = true
    }
  }, [runtimeMode, selectedRun])

  if (!selectedRun) return null

  const stateClass =
    selectedRun.state === 'active' ? 'inspect-state--active' :
    selectedRun.state === 'waitingApproval' ? 'inspect-state--waiting' :
    selectedRun.state === 'error' || selectedRun.state === 'gatewayOffline' || selectedRun.state === 'limitExceeded' || selectedRun.state === 'contextExceeded' ? 'inspect-state--error' :
    'inspect-state--done'

  const isError = selectedRun.state === 'error' || selectedRun.state === 'gatewayOffline' || selectedRun.state === 'limitExceeded' || selectedRun.state === 'contextExceeded'

  const toolColor =
    selectedRun.tool === 'claude' ? 'var(--claude-accent)' :
    selectedRun.tool === 'codex' ? 'var(--codex-accent)' :
    'var(--openclaw-accent)'

  return (
    <div className="inspect-overlay" onClick={(e) => {
      if (e.target === e.currentTarget) selectRun(undefined)
    }}>
      <div className="inspect-panel" ref={panelRef} role="dialog" aria-modal="true" aria-labelledby="inspect-title">
        {/* Header: title row + timestamps */}
        <div className="inspect-header">
          <div className="inspect-title-row">
            <div className="inspect-title-left">
              <span className="inspect-tool-dot" style={{ background: toolColor }} />
              <h3 id="inspect-title" className="inspect-project">{selectedRun.projectName}</h3>
              <span className="inspect-path">{selectedRun.workspaceShort || ''}</span>
            </div>
            <span className={`inspect-state ${stateClass}`}>{stateLabel(selectedRun.state, t)}</span>
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

        {/* Metrics grid */}
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

        {/* Origin */}
        {selectedRun.originLabel && (
          <div className="inspect-section">
            <span className="inspect-section-label">{t('drawer.origin')}</span>
            <span className="inspect-section-value">{selectedRun.originLabel}</span>
          </div>
        )}

        {/* Questions */}
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

        {/* Error */}
        {isError && (
          <div className="inspect-error">
            <span className="inspect-error-state">{selectedRun.state}</span>
            {selectedRun.errorMessage && <p className="inspect-error-msg">{selectedRun.errorMessage}</p>}
            {!selectedRun.errorMessage && selectedRun.lastTail && <p className="inspect-error-msg">{selectedRun.lastTail}</p>}
          </div>
        )}
      </div>
    </div>
  )
}
